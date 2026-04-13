use aes_gcm_siv::{
    aead::{Aead, KeyInit, OsRng},
    Aes256GcmSiv, Nonce,
};
use rand::RngCore;
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

// ─── Crypto helpers ──────────────────────────────────────────────────────

fn encrypt(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err("Key must be 32 bytes".into());
    }
    let cipher = Aes256GcmSiv::new_from_slice(key).map_err(|e| format!("Invalid key: {e}"))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

fn decrypt(ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err("Key must be 32 bytes".into());
    }
    if ciphertext.len() < 12 {
        return Err("Ciphertext too short".into());
    }
    let cipher = Aes256GcmSiv::new_from_slice(key).map_err(|e| format!("Invalid key: {e}"))?;

    let nonce = Nonce::from_slice(&ciphertext[..12]);
    cipher
        .decrypt(nonce, &ciphertext[12..])
        .map_err(|e| format!("Decryption failed: {e}"))
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
            version: 2,
            files: vec![],
        });
    }
    let enc = match fs::read(&path) {
        Ok(d) => d,
        Err(_) => {
            return Ok(Manifest {
                version: 2,
                files: vec![],
            })
        }
    };
    let dec = match decrypt(&enc, &encryption_key) {
        Ok(d) => d,
        Err(_) => {
            return Ok(Manifest {
                version: 2,
                files: vec![],
            })
        }
    };
    serde_json::from_slice(&dec).map_err(|e| format!("Invalid manifest JSON: {e}"))
}

/// Encrypt and atomically save the base manifest to disk.
#[tauri::command]
pub fn save_manifest(
    vault_path: String,
    encryption_key: Vec<u8>,
    manifest: Manifest,
) -> Result<(), String> {
    let json = serde_json::to_vec(&manifest).map_err(|e| format!("JSON serialize failed: {e}"))?;
    let enc = encrypt(&json, &encryption_key)?;
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

        // Both sides agree → nothing to do
        if local_h == remote_h {
            continue;
        }

        let kind = if base_h.is_none() {
            // File didn't exist at last sync
            match (local_h, remote_h) {
                (Some(_), None) => "upload",
                (None, Some(_)) => "download",
                _ => "conflict", // Both added with different content
            }
        } else {
            // File existed at last sync
            let local_changed = local_h != base_h;
            let remote_changed = remote_h != base_h;

            match (local_h, remote_h) {
                (None, None) => continue, // both deleted
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
                        "conflict" // Both changed differently
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
        let enc = encrypt(&data, &encryption_key)?;
        let key = format!("{}files/{}.enc", s3_prefix, rel);
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
        let key = format!("{}files/{}.enc", s3_prefix, rel);
        let response = bucket
            .get_object(&key)
            .await
            .map_err(|e| format!("Download failed for {rel}: {e}"))?;
        let dec = decrypt(response.bytes(), &encryption_key)?;

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
    let enc = encrypt(&json, &encryption_key)?;
    let key = format!("{}manifest.enc", s3_prefix);
    bucket
        .put_object(&key, &enc)
        .await
        .map_err(|e| format!("Manifest upload failed: {e}"))?;

    Ok(())
}
