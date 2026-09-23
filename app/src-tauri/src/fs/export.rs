use crate::fs::{WalkAction, path_to_string, walk_dir, walk_dir_capped};
use crate::sync::Manifest;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

#[tauri::command]
#[specta::specta]
pub async fn export_vault_zip(vault_path: String, dest_path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || export_vault_zip_blocking(&vault_path, &dest_path))
        .await
        .map_err(|e| e.to_string())?
}

fn export_vault_zip_blocking(vault_path: &str, dest_path: &str) -> Result<(), String> {
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    let root = Path::new(vault_path);
    if !root.is_dir() {
        return Err("Vault path is not a directory".into());
    }

    let file =
        std::fs::File::create(dest_path).map_err(|e| format!("Failed to create zip file: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    fn add_file(
        zip: &mut ZipWriter<std::fs::File>,
        path: &Path,
        relative: &str,
        options: SimpleFileOptions,
    ) -> Result<(), String> {
        zip.start_file(relative, options)
            .map_err(|e| format!("Failed to start file in zip: {e}"))?;
        let mut f = std::fs::File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
        std::io::copy(&mut f, zip).map_err(|e| format!("Failed to write to zip: {e}"))?;
        Ok(())
    }

    let mut error: Option<String> = None;
    walk_dir(root, &mut |item| {
        if item.name.starts_with('.') || error.is_some() {
            return WalkAction::Skip;
        }
        let relative = match item.path.strip_prefix(root) {
            Ok(rel) => path_to_string(rel.to_path_buf()),
            Err(_) => {
                error = Some("Failed to resolve path for zip".into());
                return WalkAction::Skip;
            }
        };
        if item.is_dir {
            if let Err(e) = zip.add_directory(format!("{relative}/"), options) {
                error = Some(format!("Failed to add directory to zip: {e}"));
                return WalkAction::Skip;
            }
            return WalkAction::Recurse;
        }
        if let Err(e) = add_file(&mut zip, &item.path, &relative, options) {
            error = Some(e);
        }
        WalkAction::Skip
    });
    if let Some(e) = error {
        return Err(e);
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;
    Ok(())
}

fn walk_vault_files(root: &Path) -> Vec<(String, u64)> {
    let mut result = Vec::new();
    walk_dir_capped(root, 0, crate::fs::MAX_WALK_DEPTH, &mut |item| {
        if item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        if item.is_dir {
            return WalkAction::Recurse;
        }
        if let Ok(rel) = item.path.strip_prefix(root) {
            result.push((path_to_string(rel.to_path_buf()), item.modified));
        }
        WalkAction::Skip
    });
    result
}

#[tauri::command]
#[specta::specta]
pub async fn has_unsynced_changes(
    vault_path: String,
    encryption_key: Vec<u8>,
) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || has_unsynced_changes_blocking(&vault_path, encryption_key))
        .await
        .map_err(|e| e.to_string())?
}

fn has_unsynced_changes_blocking(
    vault_path: &str,
    encryption_key: Vec<u8>,
) -> Result<bool, String> {
    struct CachedResult {
        vault_path: String,
        result: bool,
        manifest_mtime: u64,
        checked_at: Instant,
    }

    static CACHE: Mutex<Option<CachedResult>> = Mutex::new(None);
    const CACHE_TTL_SECS: u64 = 2;

    let manifest_path = Path::new(vault_path).join(".margin").join("sync-base.enc");

    let manifest_mtime = fs::metadata(&manifest_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Ok(guard) = CACHE.lock()
        && let Some(cached) = guard.as_ref()
        && cached.vault_path == vault_path
        && cached.manifest_mtime == manifest_mtime
        && cached.checked_at.elapsed().as_secs() < CACHE_TTL_SECS
    {
        return Ok(cached.result);
    }

    let manifest: Manifest = if manifest_path.exists() {
        let enc = fs::read(&manifest_path).map_err(|e| format!("Failed to read manifest: {e}"))?;
        let dec = crate::crypto::decrypt_blob(enc, encryption_key)?;
        serde_json::from_slice(&dec).map_err(|e| format!("Failed to parse manifest: {e}"))?
    } else {
        Manifest::empty()
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
        local_files
            .iter()
            .any(|(path, mtime)| match base.get(path.as_str()) {
                None => true,
                Some(&bm) if bm != *mtime => true,
                _ => false,
            })
    };

    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(CachedResult {
            vault_path: vault_path.to_string(),
            result,
            manifest_mtime,
            checked_at: Instant::now(),
        });
    }

    Ok(result)
}
