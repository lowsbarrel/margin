use hmac::{Hmac, Mac};
use s3::Bucket;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

use crate::fs::atomic_write;
use crate::s3::S3State;

mod diff;
mod transport;

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

// v2 keyed S3 objects by plaintext path, so anything older is discarded on load rather than trusted.
pub(crate) const MANIFEST_VERSION: u64 = 3;

impl Manifest {
    pub(crate) fn empty() -> Self {
        Manifest {
            version: MANIFEST_VERSION,
            files: Vec::new(),
        }
    }
}

#[derive(Serialize, specta::Type)]
pub struct SyncAction {
    pub kind: String,
    pub path: String,
}

type HmacSha256 = Hmac<Sha256>;

// Keys are HMAC-derived so listing the bucket never reveals vault paths.
fn path_to_s3_key_internal(rel_path: &str, encryption_key: &[u8]) -> String {
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(encryption_key).expect("HMAC can take key of any size");
    mac.update(rel_path.as_bytes());
    let result = mac.finalize().into_bytes();
    hex::encode(&result[..16])
}

#[tauri::command]
#[specta::specta]
pub fn path_to_s3_key(rel_path: String, encryption_key: Vec<u8>) -> String {
    path_to_s3_key_internal(&rel_path, &encryption_key)
}

fn vault_file_path(base: &Path, rel: &str) -> Result<PathBuf, String> {
    if !crate::fs::valid_rel_path(rel) {
        return Err(format!("Invalid sync path: {rel}"));
    }
    Ok(base.join(rel))
}

fn cached_bucket(state: &State<'_, S3State>) -> Result<Box<Bucket>, String> {
    let s3 = state.0.lock().map_err(|e| e.to_string())?;
    let cached = s3.as_ref().ok_or("S3 not configured")?;
    Ok(cached.bucket.clone())
}

#[tauri::command]
#[specta::specta]
pub async fn sync_delete_files(
    s3_prefix: String,
    paths: Vec<String>,
    encryption_key: Vec<u8>,
    state: State<'_, S3State>,
) -> Result<(), String> {
    let bucket = cached_bucket(&state)?;
    transport::delete_files(bucket, &s3_prefix, paths, &encryption_key).await
}

#[tauri::command]
#[specta::specta]
pub async fn hash_files_batch(
    vault_path: String,
    paths: Vec<String>,
) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || transport::hash_files_blocking(&vault_path, &paths))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
#[specta::specta]
pub fn load_manifest(vault_path: String, encryption_key: Vec<u8>) -> Result<Manifest, String> {
    let path = Path::new(&vault_path).join(".margin/sync-base.enc");
    if !path.exists() {
        return Ok(Manifest::empty());
    }
    let enc = match fs::read(&path) {
        Ok(d) => d,
        Err(_) => {
            return Ok(Manifest::empty());
        }
    };
    let dec = match crate::crypto::decrypt_blob(enc, encryption_key) {
        Ok(d) => d,
        Err(_) => {
            return Ok(Manifest::empty());
        }
    };
    let manifest: Manifest =
        serde_json::from_slice(&dec).map_err(|e| format!("Invalid manifest JSON: {e}"))?;
    if manifest.version < MANIFEST_VERSION {
        return Ok(Manifest::empty());
    }
    Ok(manifest)
}

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

#[tauri::command]
#[specta::specta]
pub fn compute_sync_actions(
    base_files: Vec<ManifestEntry>,
    local_files: Vec<ManifestEntry>,
    remote_files: Vec<ManifestEntry>,
) -> Vec<SyncAction> {
    diff::compute(base_files, local_files, remote_files)
}

#[tauri::command]
#[specta::specta]
pub fn collect_tombstones(files: Vec<ManifestEntry>) -> Vec<ManifestEntry> {
    diff::collect_tombstones(files)
}

#[tauri::command]
#[specta::specta]
pub fn merge_tombstones(a: Vec<ManifestEntry>, b: Vec<ManifestEntry>) -> Vec<ManifestEntry> {
    diff::merge_tombstones(a, b)
}

#[tauri::command]
#[specta::specta]
pub fn prune_tombstones(tombstones: Vec<ManifestEntry>, now_seconds: u32) -> Vec<ManifestEntry> {
    diff::prune_tombstones(tombstones, now_seconds)
}

#[tauri::command]
#[specta::specta]
pub async fn sync_upload_files(
    vault_path: String,
    s3_prefix: String,
    paths: Vec<String>,
    encryption_key: Vec<u8>,
    state: State<'_, S3State>,
) -> Result<(), String> {
    let bucket = cached_bucket(&state)?;
    let base = Path::new(&vault_path);
    let resolved: Vec<(String, PathBuf)> = paths
        .into_iter()
        .map(|rel| vault_file_path(base, &rel).map(|full| (rel, full)))
        .collect::<Result<_, _>>()?;

    transport::upload_files(bucket, &s3_prefix, resolved, &encryption_key).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_download_files(
    vault_path: String,
    s3_prefix: String,
    paths: Vec<String>,
    mtimes: Vec<u32>,
    encryption_key: Vec<u8>,
    state: State<'_, S3State>,
) -> Result<Vec<String>, String> {
    if mtimes.len() != paths.len() {
        return Err(format!(
            "paths/mtimes length mismatch: {} vs {}",
            paths.len(),
            mtimes.len()
        ));
    }

    let bucket = cached_bucket(&state)?;
    let base = Path::new(&vault_path);
    let resolved: Vec<(String, PathBuf, u64)> = paths
        .into_iter()
        .zip(mtimes)
        .map(|(rel, mtime)| vault_file_path(base, &rel).map(|dest| (rel, dest, mtime as u64)))
        .collect::<Result<_, _>>()?;

    transport::download_files(bucket, &s3_prefix, resolved, &encryption_key, &vault_path).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_upload_manifest(
    s3_prefix: String,
    encryption_key: Vec<u8>,
    manifest: Manifest,
    state: State<'_, S3State>,
) -> Result<(), String> {
    let bucket = cached_bucket(&state)?;
    transport::upload_manifest(bucket, &s3_prefix, &manifest, &encryption_key).await
}
