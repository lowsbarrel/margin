use super::{TrashItem, delete, empty, list_items, restore};

#[tauri::command]
#[specta::specta]
pub fn trash_list(vault_path: &str) -> Result<Vec<TrashItem>, String> {
    Ok(list_items(vault_path))
}

#[tauri::command]
#[specta::specta]
pub fn trash_restore(vault_path: &str, id: &str) -> Result<String, String> {
    restore(vault_path, id)
}

#[tauri::command]
#[specta::specta]
pub fn trash_delete(vault_path: &str, id: &str) -> Result<(), String> {
    delete(vault_path, id)
}

#[tauri::command]
#[specta::specta]
pub fn trash_empty(vault_path: &str) -> Result<u32, String> {
    Ok(empty(vault_path))
}
