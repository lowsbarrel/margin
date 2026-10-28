use crate::{RelativePath, StoreError};

pub trait HistoryStore: Send + Sync {
    fn rename_note(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError>;

    fn detach_note(
        &self,
        note: &RelativePath,
        destination: &RelativePath,
    ) -> Result<(), StoreError>;

    fn attach_snapshots(
        &self,
        source: &RelativePath,
        note: &RelativePath,
    ) -> Result<(), StoreError>;
}
