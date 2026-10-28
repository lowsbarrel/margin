use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{mem, thread};

use margin_kernel::{ChangeSink, RelativePath, StoreError, Subscription, Watcher};
use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcherApi,
    recommended_watcher,
};
use parking_lot::{Condvar, Mutex};

const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug)]
pub struct NotifyWatcher {
    root: PathBuf,
}

impl NotifyWatcher {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let root = root.canonicalize().unwrap_or(root);
        Self { root }
    }
}

impl Watcher for NotifyWatcher {
    fn watch(&self, sink: ChangeSink) -> Result<Subscription, StoreError> {
        let pending = Arc::new(Pending::default());
        let worker = thread::spawn({
            let pending = Arc::clone(&pending);
            move || flush_loop(&pending, &sink)
        });
        let callback = Arc::clone(&pending);
        let root = self.root.clone();
        let mut watcher = recommended_watcher(move |event: Result<Event, notify::Error>| {
            if let Ok(event) = event {
                callback.push(coalesced_paths(&root, &event));
            }
        })
        .map_err(|error| {
            pending.stop();
            StoreError::Other(format!("cannot watch {}: {error}", self.root.display()))
        })?;
        watcher
            .watch(&self.root, RecursiveMode::Recursive)
            .map_err(|error| {
                pending.stop();
                StoreError::Other(format!("cannot watch {}: {error}", self.root.display()))
            })?;
        Ok(Subscription::new(WatchGuard {
            _watcher: watcher,
            pending,
            worker: Some(worker),
        }))
    }
}

fn flush_loop(pending: &Pending, sink: &ChangeSink) {
    while let Some(paths) = pending.next_batch() {
        if !paths.is_empty() {
            sink(paths);
        }
    }
}

struct WatchGuard {
    _watcher: RecommendedWatcher,
    pending: Arc<Pending>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for WatchGuard {
    fn drop(&mut self) {
        self.pending.stop();
        if let Some(worker) = self.worker.take() {
            let _joined = worker.join();
        }
    }
}

#[derive(Debug, Default)]
struct Pending {
    state: Mutex<PendingState>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct PendingState {
    paths: BTreeSet<RelativePath>,
    last_event: Option<Instant>,
    stopped: bool,
}

impl Pending {
    fn push(&self, paths: Vec<RelativePath>) {
        if paths.is_empty() {
            return;
        }
        let mut state = self.state.lock();
        state.paths.extend(paths);
        state.last_event = Some(Instant::now());
        self.changed.notify_all();
    }

    fn stop(&self) {
        let mut state = self.state.lock();
        state.stopped = true;
        self.changed.notify_all();
    }

    fn next_batch(&self) -> Option<Vec<RelativePath>> {
        let mut state = self.state.lock();
        while state.paths.is_empty() && !state.stopped {
            self.changed.wait(&mut state);
        }
        while !state.stopped {
            let deadline = state
                .last_event
                .map_or_else(Instant::now, |last| last + DEBOUNCE);
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            self.changed.wait_for(&mut state, deadline - now);
        }
        if state.stopped {
            return None;
        }
        Some(mem::take(&mut state.paths).into_iter().collect())
    }
}

fn coalesced_paths(root: &Path, event: &Event) -> Vec<RelativePath> {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return Vec::new();
    }
    event
        .paths
        .iter()
        .filter_map(|path| relative_to(root, path))
        .collect()
}

fn relative_to(root: &Path, path: &Path) -> Option<RelativePath> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let relative = canonical.strip_prefix(root).ok()?;
    let mut parts: Vec<&str> = Vec::new();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return None;
        };
        let name = name.to_str()?;
        if name.starts_with('.') {
            return None;
        }
        parts.push(name);
    }
    if parts.is_empty() {
        return None;
    }
    RelativePath::new(&parts.join("/")).ok()
}
