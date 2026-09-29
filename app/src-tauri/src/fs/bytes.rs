use super::{VaultPathState, atomic_write, ensure_in_vault, vault_root};
use crate::ipc::{body, header, raw_header};
use std::fs;
use std::path::Path;
use tauri::ipc::{Request, Response};

#[tauri::command]
pub fn read_file_bytes(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<Response, String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    let bytes = fs::read(&p).map_err(|e| format!("Failed to read file: {e}"))?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub fn write_file_bytes(
    request: Request,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let path = header(&request, "x-path")?;
    let data = body(&request)?;
    let p = ensure_in_vault(&path, &vault_path_state)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    atomic_write(&p, &data)?;
    crate::index::tree::invalidate();
    crate::index::upsert_path(&vault_root(&vault_path_state), &p);
    Ok(())
}

// The destination came from the native save dialog and is outside the vault by design.
#[tauri::command]
pub fn save_file_bytes(request: Request) -> Result<(), String> {
    let path = raw_header(&request, "x-path")?;
    let data = body(&request)?;
    let dest = Path::new(&path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    atomic_write(dest, &data)
}
