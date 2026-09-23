use super::{
    MAX_WALK_DEPTH, VaultPathState, WalkAction, copy_file_at, ensure_in_vault, occupied_error,
    path_to_string, vault_root, walk_dir_capped,
};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

// The source is user-chosen and may live anywhere; only the destination is containment-checked.
#[tauri::command]
#[specta::specta]
pub fn import_external_file(
    from: &str,
    to: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let to_path = ensure_in_vault(to, &vault_path_state)?;
    copy_file_at(Path::new(from), &to_path)?;
    crate::index::tree::invalidate();
    crate::index::upsert_path(&vault_root(&vault_path_state), &to_path);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn copy_directory(
    from: String,
    to: String,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let src = ensure_in_vault(&from, &vault_path_state)?;
    let dst = ensure_in_vault(&to, &vault_path_state)?;
    tokio::task::spawn_blocking(move || copy_dir_recursive(&src, &dst))
        .await
        .map_err(|e| e.to_string())??;
    crate::index::tree::invalidate();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn import_external_directory(
    from: String,
    to: String,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let dst = ensure_in_vault(&to, &vault_path_state)?;
    let src = PathBuf::from(&from);
    tokio::task::spawn_blocking(move || import_dir_at(&src, &dst))
        .await
        .map_err(|e| e.to_string())??;
    crate::index::tree::invalidate();
    Ok(())
}

fn import_dir_at(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!(
            "Not a directory: {}",
            path_to_string(src.to_path_buf())
        ));
    }
    if dst.starts_with(src) {
        return Err("Destination is inside the source directory".into());
    }
    copy_dir_recursive(src, dst)
}

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if fs::symlink_metadata(dst).is_ok() {
        return Err(occupied_error(dst));
    }
    fs::create_dir_all(dst).map_err(|e| format!("Failed to create directory: {e}"))?;

    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut error: Option<String> = None;

    walk_dir_capped(src, 0, MAX_WALK_DEPTH, &mut |item| {
        if error.is_some() || item.is_symlink {
            return WalkAction::Skip;
        }
        let Ok(rel) = item.path.strip_prefix(src) else {
            error = Some("Failed to resolve copy destination".into());
            return WalkAction::Skip;
        };
        if item.is_dir {
            if let Err(e) = fs::create_dir_all(dst.join(rel)) {
                error = Some(format!("Failed to create directory: {e}"));
            }
            WalkAction::Recurse
        } else {
            files.push((item.path.clone(), dst.join(rel)));
            WalkAction::Skip
        }
    });
    if let Some(e) = error {
        return Err(e);
    }

    files.par_iter().try_for_each(|(s, d)| {
        fs::copy(s, d)
            .map(|_| ())
            .map_err(|e| format!("Failed to copy file: {e}"))
    })
}

#[cfg(test)]
mod tests;
