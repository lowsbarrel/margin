use hmac::{Hmac, Mac};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tauri::State;

use crate::fs::atomic_write;
use crate::s3::S3State;

// ─── Types ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct ManifestEntry {
    pub path: String,
    pub hash: String,
    pub modified: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub version: u64,
    pub files: Vec<ManifestEntry>,
}

#[derive(Serialize)]
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
pub fn path_to_s3_key(rel_path: String, encryption_key: Vec<u8>) -> String {
    path_to_s3_key_internal(&rel_path, &encryption_key)
}

fn path_to_s3_key_internal(rel_path: &str, encryption_key: &[u8]) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(encryption_key)
        .expect("HMAC can take key of any size");
    mac.update(rel_path.as_bytes());
    let result = mac.finalize().into_bytes();
    hex::encode(&result[..16])
}

/// Delete files from S3 by their relative paths (computes HMAC keys internally).
#[tauri::command]
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
    for rel in &paths {
        let key = format!("{}files/{}.enc", s3_prefix, path_to_s3_key_internal(rel, &encryption_key));
        match bucket.delete_object(&key).await {
            Ok(_) => {}
            Err(e) => {
                let err_str = format!("{e}");
                if err_str.contains("NoSuchKey") || err_str.contains("404") {
                    // Already gone, ignore
                } else {
                    return Err(format!("Delete failed for {rel}: {e}"));
                }
            }
        }
    }
    Ok(())
}

// ─── Commands ────────────────────────────────────────────────────────────

/// Compute SHA-256 hashes for a batch of files in parallel.
#[tauri::command]
pub fn hash_files_batch(vault_path: String, paths: Vec<String>) -> Result<Vec<String>, String> {
    let base = Path::new(&vault_path);
    paths
        .par_iter()
        .map(|rel| {
            let full = base.join(rel);
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
pub fn collect_tombstones(files: Vec<ManifestEntry>) -> Vec<ManifestEntry> {
    files.into_iter().filter(|e| e.deleted_at.is_some()).collect()
}

/// Merge two tombstone lists, keeping the one with the later `deleted_at`.
#[tauri::command]
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
#[tauri::command]
pub fn prune_tombstones(tombstones: Vec<ManifestEntry>, now_seconds: u64) -> Vec<ManifestEntry> {
    const TOMBSTONE_TTL: u64 = 90 * 24 * 60 * 60;
    let cutoff = now_seconds.saturating_sub(TOMBSTONE_TTL);
    tombstones
        .into_iter()
        .filter(|t| t.deleted_at.unwrap_or(0) > cutoff)
        .collect()
}

/// Read, encrypt, and upload files to S3 in a single batch.
#[tauri::command]
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

    for rel in &paths {
        let full = base.join(rel);
        let data =
            fs::read(&full).map_err(|e| format!("Failed to read {}: {e}", full.display()))?;
        let enc = crate::crypto::encrypt_blob(data, encryption_key.clone())?;
        let key = format!("{}files/{}.enc", s3_prefix, path_to_s3_key_internal(rel, &encryption_key));
        bucket
            .put_object(&key, &enc)
            .await
            .map_err(|e| format!("Upload failed for {rel}: {e}"))?;
    }

    Ok(())
}

/// Download, decrypt, and write files from S3 in a single batch.
#[tauri::command]
pub async fn sync_download_files(
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

    for rel in &paths {
        let key = format!("{}files/{}.enc", s3_prefix, path_to_s3_key_internal(rel, &encryption_key));
        let response = bucket
            .get_object(&key)
            .await
            .map_err(|e| format!("Download failed for {rel}: {e}"))?;
        let dec = crate::crypto::decrypt_blob(response.bytes().to_vec(), encryption_key.clone())?;

        let dest = base.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {e}"))?;
        }
        fs::write(&dest, &dec)
            .map_err(|e| format!("Failed to write {}: {e}", dest.display()))?;
    }

    Ok(())
}

/// Encrypt and upload the manifest to S3.
#[tauri::command]
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
