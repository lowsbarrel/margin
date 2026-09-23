use super::scan::{SWEEP_GRACE, sweep_unused};
use super::{import_file, store};
use crate::fs::{
    VaultPathState, ensure_in_vault, header, request_body, valid_rel_path, vault_root,
};
use std::path::PathBuf;
use tauri::ipc::Request;

fn attachment_dir(
    folder: &str,
    vault_path_state: &tauri::State<'_, VaultPathState>,
) -> Result<PathBuf, String> {
    if !valid_rel_path(folder) {
        return Err(format!("Invalid attachments folder: {folder}"));
    }
    ensure_in_vault(
        &format!("{}/{}", vault_root(vault_path_state), folder),
        vault_path_state,
    )
}

fn store_attachment(
    folder: &str,
    name: &str,
    bytes: &[u8],
    vault_path_state: &tauri::State<'_, VaultPathState>,
) -> Result<String, String> {
    let folder = folder.trim_matches('/');
    let file = store(&attachment_dir(folder, vault_path_state)?, name, bytes)?;
    crate::index::tree::invalidate();
    Ok(format!("{folder}/{file}"))
}

#[tauri::command]
pub fn store_attachment_bytes(
    request: Request,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<String, String> {
    let folder = header(&request, "x-folder")?;
    let name = header(&request, "x-name")?;
    store_attachment(&folder, &name, &request_body(&request)?, &vault_path_state)
}

#[tauri::command]
#[specta::specta]
pub fn import_attachment(
    from: &str,
    folder: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<String, String> {
    let folder = folder.trim_matches('/');
    let dir = attachment_dir(folder, &vault_path_state)?;
    let rel = import_file(&PathBuf::from(from), &dir, folder)?;
    crate::index::tree::invalidate();
    Ok(rel)
}

#[tauri::command]
#[specta::specta]
pub async fn sweep_unused_attachments(
    folder: String,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<u32, String> {
    let folder = folder.trim_matches('/').to_string();
    if !valid_rel_path(&folder) {
        return Err("Invalid attachments folder".into());
    }
    let root = vault_root(&vault_path_state);
    let swept = tokio::task::spawn_blocking(move || sweep_unused(&root, &folder, SWEEP_GRACE))
        .await
        .map_err(|e| e.to_string())??;
    if swept > 0 {
        crate::index::tree::invalidate();
    }
    Ok(swept)
}
