use std::collections::BTreeMap;

use margin_kernel::{RelativePath, StoreError};
use margin_sync::SnapshotStore;
use parking_lot::Mutex;

#[derive(Debug, Default)]
pub struct MemorySnapshotStore {
    entries: Mutex<BTreeMap<String, Vec<Vec<u8>>>>,
}

impl MemorySnapshotStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn count(&self, path: &str) -> usize {
        self.entries.lock().get(path).map_or(0, Vec::len)
    }

    #[must_use]
    pub fn snapshots(&self, path: &str) -> Vec<Vec<u8>> {
        self.entries.lock().get(path).cloned().unwrap_or_default()
    }
}

impl SnapshotStore for MemorySnapshotStore {
    fn save(&self, path: &RelativePath, contents: &[u8]) -> Result<(), StoreError> {
        self.entries
            .lock()
            .entry(path.to_string())
            .or_default()
            .push(contents.to_vec());
        Ok(())
    }
}
