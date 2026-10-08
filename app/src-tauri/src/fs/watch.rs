use notify::{Event, EventKind, RecursiveMode, Watcher, recommended_watcher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub struct WatcherState(pub Mutex<Option<notify::RecommendedWatcher>>);

pub struct VaultWatcherState(pub Mutex<Option<notify::RecommendedWatcher>>);

const VAULT_DEBOUNCE: Duration = Duration::from_millis(300);

struct VaultDebouncer {
    app: AppHandle,
    vault_root: PathBuf,
    last_event: Mutex<Instant>,
    pending: Mutex<Vec<PathBuf>>,
    timer_active: AtomicBool,
}

impl VaultDebouncer {
    fn new(app: AppHandle, vault_root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            app,
            vault_root,
            last_event: Mutex::new(Instant::now()),
            pending: Mutex::new(Vec::new()),
            timer_active: AtomicBool::new(false),
        })
    }

    fn notify(self: &Arc<Self>, paths: Vec<PathBuf>) {
        if !paths.is_empty()
            && let Ok(mut pending) = self.pending.lock()
        {
            pending.extend(paths);
        }
        if let Ok(mut last) = self.last_event.lock() {
            *last = Instant::now();
        }
        if self
            .timer_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let this = Arc::clone(self);
        std::thread::spawn(move || {
            loop {
                let remaining = {
                    let last = match this.last_event.lock() {
                        Ok(l) => *l,
                        Err(_) => break,
                    };
                    VAULT_DEBOUNCE.checked_sub(last.elapsed())
                };
                match remaining {
                    Some(dur) if !dur.is_zero() => std::thread::sleep(dur),
                    _ => break,
                }
            }
            this.timer_active.store(false, Ordering::Release);
            this.flush_index();
            // The cached vault tree is dropped before the frontend is told, so its refresh cannot read a stale tree.
            crate::index::tree::invalidate();
            let _ = this.app.emit("vault-fs-changed", ());
        });
    }

    fn flush_index(&self) {
        let Some(root) = self.vault_root.to_str().map(|s| s.to_string()) else {
            return;
        };
        let canonical_root = Path::new(&root).canonicalize().ok();
        let paths = match self.pending.lock() {
            Ok(mut pending) => std::mem::take(&mut *pending),
            Err(_) => return,
        };
        for path in paths {
            let Ok(resolved) = crate::fs::ensure_within(&root, &path.to_string_lossy()) else {
                continue;
            };
            if let Some(canonical_root) = &canonical_root
                && let Ok(rel) = resolved.strip_prefix(canonical_root)
                && rel.components().any(|c| {
                    c.as_os_str()
                        .to_str()
                        .map(|s| s.starts_with('.'))
                        .unwrap_or(false)
                })
            {
                continue;
            }
            if resolved.exists() {
                if resolved.is_dir() {
                    crate::index::upsert_dir(&root, &resolved);
                } else if resolved.extension().and_then(|e| e.to_str()) == Some("md") {
                    crate::index::upsert_path(&root, &resolved);
                }
            } else {
                crate::index::remove_path(&root, &resolved);
                crate::index::remove_prefix(&root, &resolved);
            }
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn watch_file(app: AppHandle, path: String) -> Result<(), String> {
    let watcher_state = app.state::<WatcherState>();
    let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;

    *guard = None;

    // Watching the file itself drops the inotify watch after an atomic-rename save, so its parent is watched instead.
    let parent = Path::new(&path)
        .parent()
        .ok_or_else(|| "File has no parent directory".to_string())?
        .to_path_buf();
    let target = PathBuf::from(&path);
    let watch_path = path.clone();
    let app_handle = app.clone();

    let mut watcher = recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_)
                    if event
                        .paths
                        .iter()
                        .any(|p| p.file_name() == target.file_name()) =>
                {
                    let _ = app_handle.emit("file-changed", &watch_path);
                }
                _ => {}
            }
        }
    })
    .map_err(|e| format!("Failed to create watcher: {e}"))?;

    watcher
        .watch(&parent, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch file: {e}"))?;

    *guard = Some(watcher);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn unwatch_file(app: AppHandle) -> Result<(), String> {
    let watcher_state = app.state::<WatcherState>();
    let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn watch_vault(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<VaultWatcherState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;

    *guard = None;

    let vault_root = Path::new(&path).to_path_buf();
    let debouncer = VaultDebouncer::new(app.clone(), vault_root.clone());

    let mut watcher = recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    let changed: Vec<PathBuf> = event
                        .paths
                        .iter()
                        .filter(|p| {
                            !p.strip_prefix(&vault_root)
                                .map(|rel| {
                                    rel.components().any(|c| {
                                        c.as_os_str()
                                            .to_str()
                                            .map(|s| s.starts_with('.'))
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false)
                        })
                        .cloned()
                        .collect();
                    if changed.is_empty() {
                        return;
                    }
                    debouncer.notify(changed);
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
#[specta::specta]
pub fn unwatch_vault(app: AppHandle) -> Result<(), String> {
    let state = app.state::<VaultWatcherState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}
