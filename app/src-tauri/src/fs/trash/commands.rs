use super::{TrashItem, delete, empty, list_items, restore};
use crate::fs::{VaultPathState, vault_root};

#[tauri::command]
#[specta::specta]
pub fn trash_list(
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<Vec<TrashItem>, String> {
    Ok(list_items(&vault_root(&vault_path_state)))
}

#[tauri::command]
#[specta::specta]
pub fn trash_restore(
    vault_path_state: tauri::State<'_, VaultPathState>,
    id: &str,
) -> Result<String, String> {
    restore(&vault_root(&vault_path_state), id)
}

#[tauri::command]
#[specta::specta]
pub fn trash_delete(
    vault_path_state: tauri::State<'_, VaultPathState>,
    id: &str,
) -> Result<(), String> {
    delete(&vault_root(&vault_path_state), id)
}

#[tauri::command]
#[specta::specta]
pub fn trash_empty(vault_path_state: tauri::State<'_, VaultPathState>) -> Result<u32, String> {
    Ok(empty(&vault_root(&vault_path_state)))
}
