use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

#[tauri::command]
pub fn export_vault_zip(vault_path: &str, dest_path: &str) -> Result<(), String> {
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let root = Path::new(vault_path);
    if !root.is_dir() {
        return Err("Vault path is not a directory".into());
    }

    let file =
        std::fs::File::create(dest_path).map_err(|e| format!("Failed to create zip file: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    fn walk_zip(
        base: &Path,
        dir: &Path,
        zip: &mut ZipWriter<std::fs::File>,
        options: SimpleFileOptions,
    ) -> Result<(), String> {
        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))?;
        for entry in entries.flatten() {
            let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .map_err(|e| format!("Path error: {e}"))?
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                zip.add_directory(format!("{relative}/"), options)
                    .map_err(|e| format!("Failed to add directory to zip: {e}"))?;
                walk_zip(base, &path, zip, options)?;
            } else {
                zip.start_file(&relative, options)
                    .map_err(|e| format!("Failed to start file in zip: {e}"))?;
                let mut f =
                    std::fs::File::open(&path).map_err(|e| format!("Failed to open file: {e}"))?;
                std::io::copy(&mut f, zip)
                    .map_err(|e| format!("Failed to write to zip: {e}"))?;
            }
        }
        Ok(())
    }

    walk_zip(root, root, &mut zip, options)?;
    zip.finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;
    Ok(())
}

// ─── Unsynced-changes check (fully Rust-side) ────────────────────────────

/// Manifest entry matching the JSON shape produced by s3sync.ts.
#[derive(Deserialize)]
struct ManifestEntry {
    path: String,
    #[allow(dead_code)]
    hash: String,
    modified: u64,
    deleted_at: Option<u64>,
}

#[derive(Deserialize)]
struct Manifest {
    #[allow(dead_code)]
    version: u64,
    files: Vec<ManifestEntry>,
}

/// Decrypt nonce‖ciphertext with AES-256-GCM-SIV (same scheme as crypto.rs).
fn decrypt_blob_inline(ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm_siv::{aead::Aead, Aes256GcmSiv, KeyInit, Nonce};
    if key.len() != 32 {
        return Err("Key must be 32 bytes".into());
    }
    if ciphertext.len() < 12 {
        return Err("Ciphertext too short".into());
    }
    let cipher =
        Aes256GcmSiv::new_from_slice(key).map_err(|e| format!("Invalid key: {e}"))?;
    let nonce = Nonce::from_slice(&ciphertext[..12]);
    cipher
        .decrypt(nonce, &ciphertext[12..])
        .map_err(|e| format!("Decryption failed: {e}"))
}

/// Collect all non-hidden files in the vault, returning (relative_path, mtime_secs).
fn walk_vault_files(root: &Path) -> Vec<(String, u64)> {
    let mut result = Vec::new();
    walk_vault_impl(root, root, &mut result);
    result
}

fn walk_vault_impl(base: &Path, dir: &Path, out: &mut Vec<(String, u64)>) {
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            walk_vault_impl(base, &entry.path(), out);
        } else {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if let Ok(rel) = entry.path().strip_prefix(base) {
                let rel_str = rel.to_string_lossy().into_owned();
                #[cfg(target_os = "windows")]
                let rel_str = rel_str.replace('\\', "/");
                out.push((rel_str, mtime));
            }
        }
    }
}

/// Check whether the vault has local changes compared to the last-synced
/// base manifest.
///
/// Caches the result for up to 2 seconds to avoid repeated full vault walks
/// when called in quick succession (e.g. on every vault-fs-changed event).
#[tauri::command]
pub fn has_unsynced_changes(vault_path: &str, encryption_key: Vec<u8>) -> Result<bool, String> {
    struct CachedResult {
        result: bool,
        manifest_mtime: u64,
        checked_at: Instant,
    }

    static CACHE: Mutex<Option<CachedResult>> = Mutex::new(None);
    const CACHE_TTL_SECS: u64 = 2;

    let manifest_path = Path::new(vault_path)
        .join(".margin")
        .join("sync-base.enc");

    // Get manifest mtime for cache invalidation
    let manifest_mtime = fs::metadata(&manifest_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Check cache
    if let Ok(guard) = CACHE.lock() {
        if let Some(ref cached) = *guard {
            if cached.manifest_mtime == manifest_mtime
                && cached.checked_at.elapsed().as_secs() < CACHE_TTL_SECS
            {
                return Ok(cached.result);
            }
        }
    }

    let manifest: Manifest = if manifest_path.exists() {
        let enc = fs::read(&manifest_path)
            .map_err(|e| format!("Failed to read manifest: {e}"))?;
        let dec = decrypt_blob_inline(&enc, &encryption_key)?;
        serde_json::from_slice(&dec)
            .map_err(|e| format!("Failed to parse manifest: {e}"))?
    } else {
        Manifest {
            version: 2,
            files: Vec::new(),
        }
    };

    let base: HashMap<&str, u64> = manifest
        .files
        .iter()
        .filter(|e| e.deleted_at.is_none())
        .map(|e| (e.path.as_str(), e.modified))
        .collect();

    let local_files = walk_vault_files(Path::new(vault_path));

    let result = if local_files.len() != base.len() {
        true
    } else {
        local_files.iter().any(|(path, mtime)| {
            match base.get(path.as_str()) {
                None => true,
                Some(&bm) if bm != *mtime => true,
                _ => false,
            }
        })
    };

    // Store in cache
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(CachedResult {
            result,
            manifest_mtime,
            checked_at: Instant::now(),
        });
    }

    Ok(result)
}
