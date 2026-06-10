use super::{VaultWatcherState, WatcherState};
use notify::{recommended_watcher, Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// Quiet-window length for coalescing a burst of vault filesystem events into a
/// single `vault-fs-changed` emission. A single logical operation (save, git
/// checkout, sync apply) produces many raw Create/Modify/Remove events; without
/// debouncing that becomes an IPC storm and repeated full re-walks on the JS
/// side. 300ms matches the frontend's own coalescing timer.
const VAULT_DEBOUNCE: Duration = Duration::from_millis(300);

/// Zero-dependency trailing-edge debouncer. `notify()` is called for every raw
/// fs event; it emits `vault-fs-changed` (empty payload) at most once per
/// ~300ms quiet window via a single coalescing timer thread.
struct VaultDebouncer {
    app: AppHandle,
    /// Instant of the most recent fs event in the current burst.
    last_event: Mutex<Instant>,
    /// True while a timer thread is alive and will eventually emit.
    timer_active: AtomicBool,
}

impl VaultDebouncer {
    fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            last_event: Mutex::new(Instant::now()),
            timer_active: AtomicBool::new(false),
        })
    }

    /// Record an fs event and ensure a timer is scheduled to emit after the
    /// quiet window. Bursts only ever keep a single timer thread alive.
    fn notify(self: &Arc<Self>) {
        if let Ok(mut last) = self.last_event.lock() {
            *last = Instant::now();
        }
        // Only spawn a timer if one isn't already running (CAS false -> true).
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
                // Sleep until the quiet window has elapsed since the last event.
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
            // Allow a new burst to schedule a fresh timer before we emit, so an
            // event arriving right after this point is not silently dropped.
            this.timer_active.store(false, Ordering::Release);
            let _ = this.app.emit("vault-fs-changed", ());
        });
    }
}

#[tauri::command]
#[specta::specta]
pub fn watch_file(app: AppHandle, path: String) -> Result<(), String> {
    let watcher_state = app.state::<WatcherState>();
    let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;

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
#[specta::specta]
pub fn unwatch_file(app: AppHandle) -> Result<(), String> {
    let watcher_state = app.state::<WatcherState>();
    let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}

/// Watch the entire vault directory recursively. Emits `"vault-fs-changed"`
/// immediately whenever a non-hidden file is created, modified or deleted.
#[tauri::command]
#[specta::specta]
pub fn watch_vault(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<VaultWatcherState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;

    *guard = None;

    let vault_root = Path::new(&path).to_path_buf();
    let debouncer = VaultDebouncer::new(app.clone());

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
                    // Coalesce bursts: emit at most once per ~300ms quiet window.
                    debouncer.notify();
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
