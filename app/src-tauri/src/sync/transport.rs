use rayon::prelude::*;
use s3::Bucket;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use super::{Manifest, path_to_s3_key_internal};

const SYNC_CONCURRENCY: usize = 8;

async fn run_bounded<T, R, F>(items: Vec<T>, mut spawn: F) -> Result<Vec<R>, String>
where
    R: Send + 'static,
    F: FnMut(&mut tokio::task::JoinSet<Result<R, String>>, T),
{
    let mut iter = items.into_iter();
    let mut tasks = tokio::task::JoinSet::new();
    for item in iter.by_ref().take(SYNC_CONCURRENCY) {
        spawn(&mut tasks, item);
    }

    let mut outputs = Vec::new();
    while let Some(result) = tasks.join_next().await {
        outputs.push(result.map_err(|e| format!("Task panicked: {e}"))??);
        if let Some(item) = iter.next() {
            spawn(&mut tasks, item);
        }
    }
    Ok(outputs)
}

pub(super) fn hash_files_blocking(
    vault_path: &str,
    paths: &[String],
) -> Result<Vec<String>, String> {
    let base = Path::new(vault_path);
    paths
        .par_iter()
        .map(|rel| {
            let full = super::vault_file_path(base, rel)?;
            let data =
                fs::read(&full).map_err(|e| format!("Failed to read {}: {e}", full.display()))?;
            let mut hasher = Sha256::new();
            hasher.update(&data);
            Ok(hex::encode(hasher.finalize()))
        })
        .collect()
}

pub(super) async fn delete_files(
    bucket: Box<Bucket>,
    s3_prefix: &str,
    paths: Vec<String>,
    encryption_key: &[u8],
) -> Result<(), String> {
    run_bounded(paths, |tasks, rel| {
        let bucket = bucket.clone();
        let key = format!(
            "{}files/{}.enc",
            s3_prefix,
            path_to_s3_key_internal(&rel, encryption_key)
        );
        tasks.spawn(async move {
            match bucket.delete_object(&key).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    let err_str = format!("{e}");
                    if err_str.contains("NoSuchKey") || err_str.contains("404") {
                        Ok(())
                    } else {
                        Err(format!("Delete failed for {rel}: {e}"))
                    }
                }
            }
        });
    })
    .await?;

    Ok(())
}

pub(super) async fn upload_files(
    bucket: Box<Bucket>,
    s3_prefix: &str,
    resolved: Vec<(String, PathBuf)>,
    encryption_key: &[u8],
) -> Result<(), String> {
    run_bounded(resolved, |tasks, (rel, full)| {
        let bucket = bucket.clone();
        let encryption_key = encryption_key.to_vec();
        let key = format!(
            "{}files/{}.enc",
            s3_prefix,
            path_to_s3_key_internal(&rel, &encryption_key)
        );
        tasks.spawn(async move {
            let data = tokio::fs::read(&full)
                .await
                .map_err(|e| format!("Failed to read {}: {e}", full.display()))?;
            let enc = crate::crypto::encrypt_blob(data, encryption_key)?;
            bucket
                .put_object(&key, &enc)
                .await
                .map_err(|e| format!("Upload failed for {rel}: {e}"))?;
            Ok(())
        });
    })
    .await?;

    Ok(())
}

pub(super) async fn download_files(
    bucket: Box<Bucket>,
    s3_prefix: &str,
    resolved: Vec<(String, PathBuf, u64)>,
    encryption_key: &[u8],
    vault_path: &str,
) -> Result<Vec<String>, String> {
    let skipped = run_bounded(resolved, |tasks, (rel, dest, mtime)| {
        let bucket = bucket.clone();
        let encryption_key = encryption_key.to_vec();
        let vault = vault_path.to_string();
        let history_path = format!("{vault_path}/{rel}");
        let key = format!(
            "{}files/{}.enc",
            s3_prefix,
            path_to_s3_key_internal(&rel, &encryption_key)
        );
        tasks.spawn(async move {
            let response = bucket
                .get_object(&key)
                .await
                .map_err(|e| format!("Download failed for {rel}: {e}"))?;
            // rust-s3 returns Ok for HTTP error statuses too, so an unchecked 404 would feed an XML error page to decrypt_blob.
            let status = response.status_code();
            if status == 404 {
                return Ok(Some(rel));
            }
            if !(200..300).contains(&status) {
                return Err(format!("Download failed for {rel}: HTTP {status}"));
            }
            let dec = crate::crypto::decrypt_blob(response.bytes().to_vec(), encryption_key)?;

            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("Failed to create directory: {e}"))?;
            }

            // This write discards a local version the user did not ask to lose, so the bytes it replaces are snapshotted first.
            if dest.exists() {
                let local = tokio::fs::read(&dest).await.unwrap_or_default();
                if local != dec {
                    let vault = vault.clone();
                    let history_path = history_path.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        crate::history::save_snapshot_inner(&vault, &history_path, &local)
                    })
                    .await;
                }
            }

            tokio::fs::write(&dest, &dec)
                .await
                .map_err(|e| format!("Failed to write {}: {e}", dest.display()))?;

            filetime::set_file_mtime(&dest, filetime::FileTime::from_unix_time(mtime as i64, 0))
                .map_err(|e| format!("Failed to set mtime for {}: {e}", dest.display()))?;
            Ok(None)
        });
    })
    .await?;

    Ok(skipped.into_iter().flatten().collect())
}

pub(super) async fn upload_manifest(
    bucket: Box<Bucket>,
    s3_prefix: &str,
    manifest: &Manifest,
    encryption_key: &[u8],
) -> Result<(), String> {
    let json = serde_json::to_vec(manifest).map_err(|e| format!("JSON serialize failed: {e}"))?;
    let enc = crate::crypto::encrypt_blob(json, encryption_key.to_vec())?;
    let key = format!("{}manifest.enc", s3_prefix);
    bucket
        .put_object(&key, &enc)
        .await
        .map_err(|e| format!("Manifest upload failed: {e}"))?;

    Ok(())
}
