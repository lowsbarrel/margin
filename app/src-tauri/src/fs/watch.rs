use super::{WatcherState, VaultWatcherState};
use notify::{recommended_watcher, Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};

#[tauri::command]
pub fn watch_file(app: AppHandle, path: String) -> Result<(), String> {
    let watcher_state = app.state::<WatcherState>();
    let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;

    // Stop existing watcher
    *guard = None;

    let watch_path = path.clone();
    let app_handle = app.clone();

    let mut watcher = recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    let _ = app_handle.emit("file-changed", &watch_path);
                }
                _ => {}
            }
        }
    })
    .map_err(|e| format!("Failed to create watcher: {e}"))?;

    watcher
        .watch(Path::new(&path), RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch file: {e}"))?;

    *guard = Some(watcher);
    Ok(())
}

#[tauri::command]
pub fn unwatch_file(app: AppHandle) -> Result<(), String> {
    let watcher_state = app.state::<WatcherState>();
    let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}

/// Watch the entire vault directory recursively. Emits `"vault-fs-changed"`
/// immediately whenever a non-hidden file is created, modified or deleted.
#[tauri::command]
pub fn watch_vault(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<VaultWatcherState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;

    // Stop any previous vault watcher
    *guard = None;

    let vault_root = Path::new(&path).to_path_buf();
    let app_handle = app.clone();

    let mut watcher = recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    // Skip hidden files/folders (. prefix) — includes .margin/, .git/, etc.
                    let all_hidden = event.paths.iter().all(|p| {
                        p.strip_prefix(&vault_root)
                            .map(|rel| {
                                rel.components().any(|c| {
                                    c.as_os_str()
                                        .to_str()
                                        .map(|s| s.starts_with('.'))
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false)
                    });
                    if all_hidden {
                        return;
                    }
                    let _ = app_handle.emit("vault-fs-changed", ());
                }
                _ => {}
            }
        }
    })
    .map_err(|e| format!("Failed to create vault watcher: {e}"))?;

    watcher
        .watch(Path::new(&path), RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch vault: {e}"))?;

    *guard = Some(watcher);
    Ok(())
}

#[tauri::command]
pub fn unwatch_vault(app: AppHandle) -> Result<(), String> {
    let state = app.state::<VaultWatcherState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}
