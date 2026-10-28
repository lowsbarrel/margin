use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use margin_kernel::{EntryKind, FileSystem, RelativePath, StoreError, UnixMillis};

use crate::domain::{
    Backlink, IndexedNote, IndexedNoteMeta, SearchHit, SearchMode, TagInfo, build_match_for,
};
use crate::error::Error;
use crate::ports::SearchIndex;

const MAX_WALK_DEPTH: usize = 10;
const MARKDOWN_EXTENSION: &str = "md";

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
        let mut paths = Vec::new();
        self.collect_notes(&RelativePath::root(), 0, &mut paths)?;

        let existing: BTreeMap<RelativePath, IndexedNoteMeta> = self
            .index
            .indexed()?
            .into_iter()
            .map(|meta| (meta.path.clone(), meta))
            .collect();

        let mut present = BTreeSet::new();
        for path in paths {
            present.insert(path.clone());
            let Ok(metadata) = self.files.metadata(&path) else {
                continue;
            };
            if existing.get(&path).is_some_and(|meta| {
                meta.modified == metadata.modified && meta.size == metadata.size
            }) {
                continue;
            }
            self.upsert_note(path, metadata.modified, metadata.size)?;
        }

        for path in existing.keys() {
            if !present.contains(path) {
                self.index.remove(path)?;
            }
        }

        Ok(u32::try_from(present.len()).unwrap_or(u32::MAX))
    }

    pub fn apply_changes(&self, changes: &[RelativePath]) -> Result<(), Error> {
        for path in changes {
            if path.is_hidden() {
                continue;
            }
            match self.files.metadata(path) {
                Ok(metadata) if metadata.kind == EntryKind::Directory => {
                    self.index_subtree(path)?;
                }
                Ok(metadata) if path.extension() == Some(MARKDOWN_EXTENSION) => {
                    self.upsert_note(path.clone(), metadata.modified, metadata.size)?;
                }
                Ok(_) => {}
                Err(StoreError::NotFound(_) | StoreError::OutsideRoot(_)) => {
                    self.index.remove(path)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
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

    fn index_subtree(&self, directory: &RelativePath) -> Result<(), Error> {
        let mut paths = Vec::new();
        self.collect_notes(directory, 0, &mut paths)?;
        for path in paths {
            let Ok(metadata) = self.files.metadata(&path) else {
                continue;
            };
            self.upsert_note(path, metadata.modified, metadata.size)?;
        }
        Ok(())
    }

    fn upsert_note(
        &self,
        path: RelativePath,
        modified: UnixMillis,
        size: u64,
    ) -> Result<(), Error> {
        let body = self.read_body(&path);
        self.index.upsert(&IndexedNote {
            path,
            body,
            modified,
            size,
        })?;
        Ok(())
    }

    fn read_body(&self, path: &RelativePath) -> String {
        let bytes = self.files.read(path).unwrap_or_default();
        String::from_utf8(bytes).unwrap_or_default()
    }

    fn collect_notes(
        &self,
        directory: &RelativePath,
        depth: usize,
        out: &mut Vec<RelativePath>,
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
                EntryKind::File if path.extension() == Some(MARKDOWN_EXTENSION) => out.push(path),
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
