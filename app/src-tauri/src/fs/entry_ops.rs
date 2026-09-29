use super::{
    VaultPathState, ensure_in_vault, ensure_in_vault_keep_name, path_to_string, trash, vault_root,
};
use std::fs;
use std::path::Path;

#[tauri::command]
#[specta::specta]
pub fn delete_entry(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    let was_dir = p.is_dir();
    let root = vault_root(&vault_path_state);
    trash::trash_entry(&root, &p)?;
    crate::index::tree::invalidate();
    if was_dir {
        crate::index::remove_prefix(&root, &p);
    } else {
        crate::index::remove_path(&root, &p);
    }
    Ok(())
}

pub(crate) fn occupied_error(to: &Path) -> String {
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path_to_string(to.to_path_buf()));
    format!("\"{name}\" already exists")
}

fn destination_conflicts(from: &Path, to: &Path) -> bool {
    fs::symlink_metadata(to).is_ok() && !same_entry(from, to)
}

// macOS and Windows see the two spellings as one entry, so the names are compared case-folded.
fn same_entry(from: &Path, to: &Path) -> bool {
    if from.parent() != to.parent() {
        return false;
    }
    let (Some(from_name), Some(to_name)) = (from.file_name(), to.file_name()) else {
        return false;
    };
    if from_name == to_name {
        return true;
    }
    if from_name.to_string_lossy().to_lowercase() != to_name.to_string_lossy().to_lowercase() {
        return false;
    }
    match fs::read_dir(from.parent().unwrap_or(Path::new("."))) {
        Ok(entries) => !entries
            .flatten()
            .any(|e| e.file_name().as_os_str() == to_name),
        Err(_) => false,
    }
}

fn rename_entry_at(from: &Path, to: &Path) -> Result<(), String> {
    if destination_conflicts(from, to) {
        return Err(occupied_error(to));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    fs::rename(from, to).map_err(|e| format!("Failed to rename: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn rename_entry(
    from: &str,
    to: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let from_path = ensure_in_vault(from, &vault_path_state)?;
    let to_path = ensure_in_vault_keep_name(to, &vault_path_state)?;
    let was_dir = from_path.is_dir();
    rename_entry_at(&from_path, &to_path)?;
    crate::index::tree::invalidate();
    let root = vault_root(&vault_path_state);
    if was_dir {
        crate::index::remove_prefix(&root, &from_path);
    } else {
        crate::index::remove_path(&root, &from_path);
        crate::index::upsert_path(&root, &to_path);
    }
    Ok(())
}

pub(crate) fn copy_file_at(from: &Path, to: &Path) -> Result<(), String> {
    if fs::symlink_metadata(to).is_ok() {
        return Err(occupied_error(to));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    fs::copy(from, to).map_err(|e| format!("Failed to copy file: {e}"))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn copy_file(
    from: &str,
    to: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let from_path = ensure_in_vault(from, &vault_path_state)?;
    let to_path = ensure_in_vault(to, &vault_path_state)?;
    copy_file_at(&from_path, &to_path)?;
    crate::index::tree::invalidate();
    crate::index::upsert_path(&vault_root(&vault_path_state), &to_path);
    Ok(())
}

#[cfg(test)]
mod tests;
