use super::{VaultPathState, ensure_in_vault, path_to_string};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize, Clone, specta::Type)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub path: String,
    #[specta(type = u32)]
    pub modified: u64,
}

#[derive(Serialize, Clone, specta::Type)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    #[specta(type = u32)]
    pub modified: u64,
    #[specta(type = u32)]
    pub depth: usize,
}

#[derive(Serialize, Clone, specta::Type)]
pub struct FileMetadata {
    #[specta(type = f64)]
    pub size: u64,
    #[specta(type = u32)]
    pub modified: u64,
}

#[tauri::command]
#[specta::specta]
pub fn list_directory(path: &str) -> Result<Vec<FsEntry>, String> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Ok(vec![]);
    }
    let mut entries = Vec::new();
    let dir = fs::read_dir(p).map_err(|e| format!("Failed to read directory: {e}"))?;
    for entry in dir.flatten() {
        let name = entry
            .file_name()
            .into_string()
            .unwrap_or_else(|s| s.to_string_lossy().into_owned());
        if name.starts_with('.') {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        entries.push(FsEntry {
            name,
            is_dir: entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false),
            path: path_to_string(entry.path()),
            modified,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

fn modified_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tauri::command]
#[specta::specta]
pub fn create_directory(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    fs::create_dir_all(&p).map_err(|e| format!("Failed to create directory: {e}"))?;
    crate::index::tree::invalidate();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn file_exists(path: &str, vault_path_state: tauri::State<'_, VaultPathState>) -> bool {
    ensure_in_vault(path, &vault_path_state)
        .map(|p| p.exists())
        .unwrap_or(false)
}

#[tauri::command]
#[specta::specta]
pub fn file_metadata(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<FileMetadata, String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    let meta = fs::metadata(&p).map_err(|e| format!("Failed to read file metadata: {e}"))?;
    Ok(FileMetadata {
        size: meta.len(),
        modified: modified_secs(&meta),
    })
}
