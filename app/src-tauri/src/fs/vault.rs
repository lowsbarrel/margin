use super::{VaultPathState, trash};
use std::fs;
use std::path::Path;

#[tauri::command]
#[specta::specta]
pub fn set_vault_directory(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        fs::create_dir_all(p).map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let margin_dir = p.join(".margin").join("docs");
    fs::create_dir_all(&margin_dir).map_err(|e| format!("Failed to create .margin/docs: {e}"))?;
    if let Ok(mut vp) = vault_path_state.0.lock() {
        *vp = path.to_string();
    }
    let root = path.to_string();
    std::thread::spawn(move || {
        if let Err(e) = crate::index::rebuild(&root) {
            eprintln!("Initial search index build failed: {e}");
        }
    });
    let purge_root = path.to_string();
    std::thread::spawn(move || {
        let removed = trash::purge_older_than(&purge_root, trash::PURGE_AGE_MS);
        if removed > 0 {
            eprintln!("Purged {removed} expired trash item(s)");
        }
        let pruned = crate::history::prune_orphans(&purge_root);
        if pruned > 0 {
            eprintln!("Pruned {pruned} orphaned history dir(s)");
        }
    });
    Ok(())
}
