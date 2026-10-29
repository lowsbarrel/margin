use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use margin_kernel::{EntryKind, FileSystem, Metadata, RelativePath, StoreError};

use crate::domain::{
    Backlink, IndexedNote, IndexedNoteMeta, SearchHit, SearchMode, TagInfo, build_match_for,
};
use crate::error::Error;
use crate::ports::SearchIndex;

const MAX_WALK_DEPTH: usize = 10;
const MARKDOWN_EXTENSION: &str = "md";
const UPSERT_BATCH: usize = 256;

pub struct SearchService {
    index: Arc<dyn SearchIndex>,
    files: Arc<dyn FileSystem>,
}

impl SearchService {
    #[must_use]
    pub fn new(index: Arc<dyn SearchIndex>, files: Arc<dyn FileSystem>) -> Self {
        Self { index, files }
    }

    pub fn rebuild(&self) -> Result<u32, Error> {
        let mut notes = Vec::new();
        self.collect_notes(&RelativePath::root(), 0, &mut notes)?;
        let mut existing: BTreeMap<RelativePath, IndexedNoteMeta> = self
            .index
            .indexed()?
            .into_iter()
            .map(|meta| (meta.path.clone(), meta))
            .collect();
        let count = u32::try_from(notes.len()).unwrap_or(u32::MAX);
        let stale = notes.into_iter().filter(|(path, metadata)| {
            existing
                .remove(path)
                .is_none_or(|meta| meta.modified != metadata.modified || meta.size != metadata.size)
        });
        self.upsert_in_batches(stale)?;
        for path in existing.keys() {
            self.index.remove(path)?;
        }
        Ok(count)
    }

    pub fn apply_changes(&self, changes: &[RelativePath]) -> Result<(), Error> {
        let mut touched = Vec::new();
        for path in changes {
            if path.is_hidden() {
                continue;
            }
            match self.files.metadata(path) {
                Ok(metadata) if metadata.kind == EntryKind::Directory => {
                    self.collect_notes(path, 0, &mut touched)?;
                }
                Ok(metadata) if path.extension() == Some(MARKDOWN_EXTENSION) => {
                    touched.push((path.clone(), metadata));
                }
                Ok(_) => {}
                Err(StoreError::NotFound(_) | StoreError::OutsideRoot(_)) => {
                    self.index.remove(path)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        self.upsert_in_batches(touched)
    }

    pub fn search(
        &self,
        query: &str,
        mode: SearchMode,
        limit: u32,
    ) -> Result<Vec<SearchHit>, Error> {
        Ok(self.index.search(&build_match_for(query, mode), limit)?)
    }

    pub fn tags(&self) -> Result<Vec<TagInfo>, Error> {
        let mut tags = self.index.tags()?;
        tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
        Ok(tags)
    }

    pub fn backlinks(&self, path: &RelativePath) -> Result<Vec<Backlink>, Error> {
        Ok(self.index.backlinks(path)?)
    }

    pub fn clear(&self) -> Result<(), Error> {
        Ok(self.index.clear()?)
    }

    fn upsert_in_batches(
        &self,
        notes: impl IntoIterator<Item = (RelativePath, Metadata)>,
    ) -> Result<(), Error> {
        let mut batch = Vec::with_capacity(UPSERT_BATCH);
        for (path, metadata) in notes {
            let body =
                String::from_utf8(self.files.read(&path).unwrap_or_default()).unwrap_or_default();
            batch.push(IndexedNote {
                path,
                body,
                modified: metadata.modified,
                size: metadata.size,
            });
            if batch.len() == UPSERT_BATCH {
                self.index.upsert(&batch)?;
                batch.clear();
            }
        }
        if !batch.is_empty() {
            self.index.upsert(&batch)?;
        }
        Ok(())
    }

    fn collect_notes(
        &self,
        directory: &RelativePath,
        depth: usize,
        out: &mut Vec<(RelativePath, Metadata)>,
    ) -> Result<(), Error> {
        if depth >= MAX_WALK_DEPTH {
            return Ok(());
        }
        let mut entries = self.files.list(directory)?;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        for entry in entries {
            if entry.name.starts_with('.') {
                continue;
            }
            let path = directory.join(&entry.name)?;
            match entry.metadata.kind {
                EntryKind::Directory => self.collect_notes(&path, depth + 1, out)?,
                EntryKind::File if path.extension() == Some(MARKDOWN_EXTENSION) => {
                    out.push((path, entry.metadata));
                }
                EntryKind::File | EntryKind::Symlink => {}
            }
        }
        Ok(())
    }
}

impl fmt::Debug for SearchService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchService").finish_non_exhaustive()
    }
}
