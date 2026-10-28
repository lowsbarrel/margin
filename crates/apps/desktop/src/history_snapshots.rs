use std::sync::Arc;

use margin_history::History;
use margin_kernel::{RelativePath, StoreError};
use margin_sync::SnapshotStore;

#[derive(Debug)]
pub(crate) struct HistorySnapshots {
    history: Arc<History>,
}

impl HistorySnapshots {
    pub(crate) const fn new(history: Arc<History>) -> Self {
        Self { history }
    }
}

impl SnapshotStore for HistorySnapshots {
    fn save(&self, path: &RelativePath, contents: &[u8]) -> Result<(), StoreError> {
        self.history
            .save(path, contents)
            .map(drop)
            .map_err(|error| StoreError::Other(error.to_string()))
    }
}
