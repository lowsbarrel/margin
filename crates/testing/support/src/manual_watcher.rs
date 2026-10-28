use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use margin_kernel::{ChangeSink, RelativePath, StoreError, Subscription, Watcher};
use parking_lot::Mutex;

#[derive(Default)]
pub struct ManualWatcher {
    sinks: Arc<Mutex<BTreeMap<u64, ChangeSink>>>,
    next: AtomicU64,
}

impl ManualWatcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trigger(&self, paths: &[RelativePath]) {
        for sink in self.sinks.lock().values() {
            sink(paths.to_vec());
        }
    }

    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.sinks.lock().len()
    }
}

impl fmt::Debug for ManualWatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManualWatcher")
            .field("subscriptions", &self.subscription_count())
            .finish_non_exhaustive()
    }
}

impl Watcher for ManualWatcher {
    fn watch(&self, sink: ChangeSink) -> Result<Subscription, StoreError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.sinks.lock().insert(id, sink);
        Ok(Subscription::new(SinkSlot {
            sinks: Arc::clone(&self.sinks),
            id,
        }))
    }
}

struct SinkSlot {
    sinks: Arc<Mutex<BTreeMap<u64, ChangeSink>>>,
    id: u64,
}

impl Drop for SinkSlot {
    fn drop(&mut self) {
        self.sinks.lock().remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("the test path is valid")
    }

    #[test]
    fn delivers_triggered_paths_to_every_subscriber() {
        let watcher = ManualWatcher::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let first = Arc::clone(&seen);
        let second = Arc::clone(&seen);
        let _first = watcher
            .watch(Box::new(move |paths| first.lock().extend(paths)))
            .expect("watching succeeds");
        let _second = watcher
            .watch(Box::new(move |paths| second.lock().extend(paths)))
            .expect("watching succeeds");
        watcher.trigger(&[path("a.md"), path("b.md")]);
        let mut seen = seen.lock().clone();
        seen.sort();
        assert_eq!(
            seen,
            [path("a.md"), path("a.md"), path("b.md"), path("b.md")]
        );
    }

    #[test]
    fn dropping_a_subscription_stops_delivery() {
        let watcher = ManualWatcher::new();
        let subscription = watcher
            .watch(Box::new(|_paths| panic!("no event is expected")))
            .expect("watching succeeds");
        assert_eq!(watcher.subscription_count(), 1);
        subscription.cancel();
        assert_eq!(watcher.subscription_count(), 0);
        watcher.trigger(&[path("a.md")]);
    }
}
