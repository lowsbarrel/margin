use margin_kernel::{RelativePath, StoreError};

use crate::domain::{Backlink, IndexedNote, IndexedNoteMeta, SearchHit, TagInfo};

pub trait SearchIndex: Send + Sync {
    fn upsert(&self, notes: &[IndexedNote]) -> Result<(), StoreError>;

    fn remove(&self, path: &RelativePath) -> Result<(), StoreError>;

    fn rename(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError>;

    fn search(&self, match_query: &str, limit: u32) -> Result<Vec<SearchHit>, StoreError>;

    fn tags(&self) -> Result<Vec<TagInfo>, StoreError>;

    fn backlinks(&self, path: &RelativePath) -> Result<Vec<Backlink>, StoreError>;

    fn indexed(&self) -> Result<Vec<IndexedNoteMeta>, StoreError>;

    fn clear(&self) -> Result<(), StoreError>;
}
