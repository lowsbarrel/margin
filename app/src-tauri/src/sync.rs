use hmac::{Hmac, Mac};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use tauri::State;

use crate::fs::atomic_write;
use crate::s3::S3State;

// ─── Types ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, specta::Type)]
pub struct ManifestEntry {
    pub path: String,
    pub hash: String,
    #[specta(type = u32)]
    pub modified: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<u32>)]
    pub deleted_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, specta::Type)]
pub struct Manifest {
    #[specta(type = u32)]
    pub version: u64,
    pub files: Vec<ManifestEntry>,
}

#[derive(Serialize, specta::Type)]
pub struct SyncAction {
    pub kind: String,
    pub path: String,
}

// ─── Path → S3 key mapping (HMAC-SHA256) ──────────────────────────────────

type HmacSha256 = Hmac<Sha256>;

/// Map a relative vault path to an opaque S3 object key.
/// Uses HMAC-SHA256 with the encryption key to derive a deterministic 32-char
/// hex identifier. Same path always maps to same key — no lookup table needed.
#[tauri::command]
#[specta::specta]
pub fn path_to_s3_key(rel_path: String, encryption_key: Vec<u8>) -> String {
    path_to_s3_key_internal(&rel_path, &encryption_key)
}

fn path_to_s3_key_internal(rel_path: &str, encryption_key: &[u8]) -> String {
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(encryption_key).expect("HMAC can take key of any size");
    mac.update(rel_path.as_bytes());
    let result = mac.finalize().into_bytes();
    hex::encode(&result[..16])
}

fn vault_file_path(base: &Path, rel: &str) -> Result<PathBuf, String> {
    if rel.is_empty() || rel.contains('\\') {
        return Err(format!("Invalid sync path: {rel}"));
    }

    for component in Path::new(rel).components() {
        match component {
            Component::Normal(name) => {
                if name
                    .to_str()
                    .map(|s| s.is_empty() || s.starts_with('.'))
                    .unwrap_or(true)
                {
                    return Err(format!("Invalid sync path: {rel}"));
                }
            }
            _ => return Err(format!("Invalid sync path: {rel}")),
        }
    }

    Ok(base.join(rel))
}

/// Maximum number of concurrent per-object S3 operations in a sync batch.
const SYNC_CONCURRENCY: usize = 8;

/// Delete files from S3 by their relative paths (computes HMAC keys internally).
#[tauri::command]
#[specta::specta]
pub async fn sync_delete_files(
    s3_prefix: String,
    paths: Vec<String>,
    encryption_key: Vec<u8>,
    state: State<'_, S3State>,
) -> Result<(), String> {
    let bucket = {
        let s3 = state.0.lock().map_err(|e| e.to_string())?;
        let cached = s3.as_ref().ok_or("S3 not configured")?;
        cached.bucket.clone()
    };

    let mut iter = paths.into_iter();
    let mut tasks = tokio::task::JoinSet::new();

    let spawn_delete = |tasks: &mut tokio::task::JoinSet<Result<(), String>>, rel: String| {
        let bucket = bucket.clone();
        let key = format!(
            "{}files/{}.enc",
            s3_prefix,
            path_to_s3_key_internal(&rel, &encryption_key)
        );
        tasks.spawn(async move {
            match bucket.delete_object(&key).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    let err_str = format!("{e}");
                    if err_str.contains("NoSuchKey") || err_str.contains("404") {
                        // Already gone, ignore
                        Ok(())
                    } else {
                        Err(format!("Delete failed for {rel}: {e}"))
                    }
                }
            }
        });
    };

    // Prime the pool with up to SYNC_CONCURRENCY tasks.
    for rel in iter.by_ref().take(SYNC_CONCURRENCY) {
        spawn_delete(&mut tasks, rel);
    }

    while let Some(result) = tasks.join_next().await {
        result.map_err(|e| format!("Task panicked: {e}"))??;
        if let Some(rel) = iter.next() {
            spawn_delete(&mut tasks, rel);
        }
    }

    Ok(())
}

// ─── Commands ────────────────────────────────────────────────────────────

/// Compute SHA-256 hashes for a batch of files in parallel.
#[tauri::command]
#[specta::specta]
pub fn hash_files_batch(vault_path: String, paths: Vec<String>) -> Result<Vec<String>, String> {
    let base = Path::new(&vault_path);
    paths
        .par_iter()
        .map(|rel| {
            let full = vault_file_path(base, rel)?;
            let data =
                fs::read(&full).map_err(|e| format!("Failed to read {}: {e}", full.display()))?;
            let mut hasher = Sha256::new();
            hasher.update(&data);
            Ok(hex::encode(hasher.finalize()))
        })
        .collect()
}

/// Load and decrypt the local base manifest, returning a default if missing.
#[tauri::command]
#[specta::specta]
pub fn load_manifest(vault_path: String, encryption_key: Vec<u8>) -> Result<Manifest, String> {
    let path = Path::new(&vault_path).join(".margin/sync-base.enc");
    if !path.exists() {
        return Ok(Manifest {
            version: 3,
            files: vec![],
        });
    }
    let enc = match fs::read(&path) {
        Ok(d) => d,
        Err(_) => {
            return Ok(Manifest {
                version: 3,
                files: vec![],
            })
        }
    };
    let dec = match crate::crypto::decrypt_blob(enc, encryption_key) {
        Ok(d) => d,
        Err(_) => {
            return Ok(Manifest {
                version: 3,
                files: vec![],
            })
        }
    };
    let manifest: Manifest =
        serde_json::from_slice(&dec).map_err(|e| format!("Invalid manifest JSON: {e}"))?;
    // Discard legacy v2 manifests — they used plaintext S3 keys
    if manifest.version < 3 {
        return Ok(Manifest {
            version: 3,
            files: vec![],
        });
    }
    Ok(manifest)
}

/// Encrypt and atomically save the base manifest to disk.
#[tauri::command]
#[specta::specta]
pub fn save_manifest(
    vault_path: String,
    encryption_key: Vec<u8>,
    manifest: Manifest,
) -> Result<(), String> {
    let json = serde_json::to_vec(&manifest).map_err(|e| format!("JSON serialize failed: {e}"))?;
    let enc = crate::crypto::encrypt_blob(json, encryption_key)?;
    let dir = Path::new(&vault_path).join(".margin");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create .margin dir: {e}"))?;
    atomic_write(&dir.join("sync-base.enc"), &enc)
}

/// 3-way diff: compare base, local , and remote manifests to produce sync actions.
#[tauri::command]
#[specta::specta]
pub fn compute_sync_actions(
    base_files: Vec<ManifestEntry>,
    local_files: Vec<ManifestEntry>,
    remote_files: Vec<ManifestEntry>,
) -> Vec<SyncAction> {
    let base: HashMap<&str, &ManifestEntry> =
        base_files.iter().map(|e| (e.path.as_str(), e)).collect();
    let local: HashMap<&str, &ManifestEntry> =
        local_files.iter().map(|e| (e.path.as_str(), e)).collect();
    let remote: HashMap<&str, &ManifestEntry> =
        remote_files.iter().map(|e| (e.path.as_str(), e)).collect();

    let all_paths: HashSet<&str> = base
        .keys()
        .chain(local.keys())
        .chain(remote.keys())
        .copied()
        .collect();

    let effective_hash = |map: &HashMap<&str, &ManifestEntry>, path: &str| -> Option<String> {
        match map.get(path) {
            Some(e) if e.deleted_at.is_none() => Some(e.hash.clone()),
            _ => None,
        }
    };

    let mut actions = Vec::new();
    for path in all_paths {
        let base_h = effective_hash(&base, path);
        let local_h = effective_hash(&local, path);
        let remote_h = effective_hash(&remote, path);

        if local_h == remote_h {
            continue;
        }

        let kind = if base_h.is_none() {
            match (local_h, remote_h) {
                (Some(_), None) => "upload",
                (None, Some(_)) => "download",
                _ => "conflict",
            }
        } else {
            let local_changed = local_h != base_h;
            let remote_changed = remote_h != base_h;

            match (local_h, remote_h) {
                (None, None) => continue,
                (None, _) => {
                    if remote_changed {
                        "conflict-delete-local"
                    } else {
                        "delete-remote"
                    }
                }
                (_, None) => {
                    if local_changed {
                        "conflict-delete-remote"
                    } else {
                        "delete-local"
                    }
                }
                _ => {
                    if local_changed && !remote_changed {
                        "upload"
                    } else if !local_changed && remote_changed {
                        "download"
                    } else {
                        "conflict"
                    }
                }
            }
        };

        actions.push(SyncAction {
            kind: kind.to_string(),
            path: path.to_string(),
        });
    }

    actions
}

/// Return only entries that have a `deleted_at` timestamp.
#[tauri::command]
#[specta::specta]
pub fn collect_tombstones(files: Vec<ManifestEntry>) -> Vec<ManifestEntry> {
    files
        .into_iter()
        .filter(|e| e.deleted_at.is_some())
        .collect()
}

/// Merge two tombstone lists, keeping the one with the later `deleted_at`.
#[tauri::command]
#[specta::specta]
pub fn merge_tombstones(a: Vec<ManifestEntry>, b: Vec<ManifestEntry>) -> Vec<ManifestEntry> {
    let mut map: HashMap<String, ManifestEntry> = HashMap::new();
    for entry in a.into_iter().chain(b) {
        let existing = map.get(&entry.path);
        if existing.is_none()
            || entry.deleted_at.unwrap_or(0) > existing.unwrap().deleted_at.unwrap_or(0)
        {
            map.insert(entry.path.clone(), entry);
        }
    }
    map.into_values().collect()
}

/// Prune tombstones older than 90 days.
///
/// `now_seconds` is a u32 (unix seconds) so specta can export it; widened to u64
/// internally to match the manifest's `deleted_at` timestamps.
#[tauri::command]
#[specta::specta]
pub fn prune_tombstones(tombstones: Vec<ManifestEntry>, now_seconds: u32) -> Vec<ManifestEntry> {
    const TOMBSTONE_TTL: u64 = 90 * 24 * 60 * 60;
    let cutoff = (now_seconds as u64).saturating_sub(TOMBSTONE_TTL);
    tombstones
        .into_iter()
        .filter(|t| t.deleted_at.unwrap_or(0) > cutoff)
        .collect()
}

/// Read, encrypt, and upload files to S3 in a single batch.
#[tauri::command]
#[specta::specta]
pub async fn sync_upload_files(
    vault_path: String,
    s3_prefix: String,
    paths: Vec<String>,
    encryption_key: Vec<u8>,
    state: State<'_, S3State>,
) -> Result<(), String> {
    let bucket = {
        let s3 = state.0.lock().map_err(|e| e.to_string())?;
        let cached = s3.as_ref().ok_or("S3 not configured")?;
        cached.bucket.clone()
    };
    let base = Path::new(&vault_path);

    // Resolve every path up front so an invalid path fails fast before any I/O.
    let resolved: Vec<(String, PathBuf)> = paths
        .into_iter()
        .map(|rel| vault_file_path(base, &rel).map(|full| (rel, full)))
        .collect::<Result<_, _>>()?;

    let mut iter = resolved.into_iter();
    let mut tasks = tokio::task::JoinSet::new();

    let spawn_upload =
        |tasks: &mut tokio::task::JoinSet<Result<(), String>>, rel: String, full: PathBuf| {
            let bucket = bucket.clone();
            let encryption_key = encryption_key.clone();
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
        };

    for (rel, full) in iter.by_ref().take(SYNC_CONCURRENCY) {
        spawn_upload(&mut tasks, rel, full);
    }

    while let Some(result) = tasks.join_next().await {
        result.map_err(|e| format!("Task panicked: {e}"))??;
        if let Some((rel, full)) = iter.next() {
            spawn_upload(&mut tasks, rel, full);
        }
    }

    Ok(())
}

/// Download, decrypt, and write files from S3 in a single batch.
///
/// `paths` and `mtimes` are parallel arrays: after each file is written its
/// modification time is set to `mtimes[i]` (Unix seconds) so the local mtime
/// matches the manifest entry and the file is not seen as locally changed on
/// the next sync. This folds the previously separate per-file `set_mtime` IPC
/// call into this single batch command.
///
/// `mtimes` are u32 unix seconds (specta cannot export u64); each is widened to
/// u64 internally to match the manifest timestamps and `filetime`.
#[tauri::command]
#[specta::specta]
pub async fn sync_download_files(
    vault_path: String,
    s3_prefix: String,
    paths: Vec<String>,
    mtimes: Vec<u32>,
    encryption_key: Vec<u8>,
    state: State<'_, S3State>,
) -> Result<(), String> {
    if mtimes.len() != paths.len() {
        return Err(format!(
            "paths/mtimes length mismatch: {} vs {}",
            paths.len(),
            mtimes.len()
        ));
    }

    let bucket = {
        let s3 = state.0.lock().map_err(|e| e.to_string())?;
        let cached = s3.as_ref().ok_or("S3 not configured")?;
        cached.bucket.clone()
    };
    let base = Path::new(&vault_path);

    // Resolve every destination up front so an invalid path fails fast.
    let resolved: Vec<(String, PathBuf, u64)> = paths
        .into_iter()
        .zip(mtimes)
        .map(|(rel, mtime)| vault_file_path(base, &rel).map(|dest| (rel, dest, mtime as u64)))
        .collect::<Result<_, _>>()?;

    let mut iter = resolved.into_iter();
    let mut tasks = tokio::task::JoinSet::new();

    let spawn_download = |tasks: &mut tokio::task::JoinSet<Result<(), String>>,
                          rel: String,
                          dest: PathBuf,
                          mtime: u64| {
        let bucket = bucket.clone();
        let encryption_key = encryption_key.clone();
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
            let dec = crate::crypto::decrypt_blob(response.bytes().to_vec(), encryption_key)?;

            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("Failed to create directory: {e}"))?;
            }
            tokio::fs::write(&dest, &dec)
                .await
                .map_err(|e| format!("Failed to write {}: {e}", dest.display()))?;

            // Restore the manifest mtime so the file isn't flagged as locally
            // changed on the next sync.
            filetime::set_file_mtime(&dest, filetime::FileTime::from_unix_time(mtime as i64, 0))
                .map_err(|e| format!("Failed to set mtime for {}: {e}", dest.display()))?;
            Ok(())
        });
    };

    for (rel, dest, mtime) in iter.by_ref().take(SYNC_CONCURRENCY) {
        spawn_download(&mut tasks, rel, dest, mtime);
    }

    while let Some(result) = tasks.join_next().await {
        result.map_err(|e| format!("Task panicked: {e}"))??;
        if let Some((rel, dest, mtime)) = iter.next() {
            spawn_download(&mut tasks, rel, dest, mtime);
        }
    }

    Ok(())
}

/// Encrypt and upload the manifest to S3.
#[tauri::command]
#[specta::specta]
pub async fn sync_upload_manifest(
    s3_prefix: String,
    encryption_key: Vec<u8>,
    manifest: Manifest,
    state: State<'_, S3State>,
) -> Result<(), String> {
    let bucket = {
        let s3 = state.0.lock().map_err(|e| e.to_string())?;
        let cached = s3.as_ref().ok_or("S3 not configured")?;
        cached.bucket.clone()
    };

    let json = serde_json::to_vec(&manifest).map_err(|e| format!("JSON serialize failed: {e}"))?;
    let enc = crate::crypto::encrypt_blob(json, encryption_key)?;
    let key = format!("{}manifest.enc", s3_prefix);
    bucket
        .put_object(&key, &enc)
        .await
        .map_err(|e| format!("Manifest upload failed: {e}"))?;

    Ok(())
}
