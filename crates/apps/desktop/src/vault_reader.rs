use std::sync::Arc;

use margin_ask::{Error, LineMatch, RecentNote, SearchHit, SearchMode, TagSummary, VaultReader};
use margin_files::{FilesService, WalkAction, WalkItem};
use margin_kernel::{FileSystem, RelativePath, StoreError, UnixMillis};
use margin_search::{SearchMode as IndexMode, SearchService};
use regex::Regex;

#[derive(Debug)]
pub(crate) struct VaultBridge {
    files: Arc<dyn FileSystem>,
    listing: Arc<FilesService>,
    search: Arc<SearchService>,
}

impl VaultBridge {
    pub(crate) const fn new(
        files: Arc<dyn FileSystem>,
        listing: Arc<FilesService>,
        search: Arc<SearchService>,
    ) -> Self {
        Self {
            files,
            listing,
            search,
        }
    }

    fn notes_under(&self, folder: &RelativePath) -> Vec<WalkItem> {
        let mut notes = Vec::new();
        margin_files::walk(self.files.as_ref(), folder, &mut |item: &WalkItem| {
            if item.is_dir {
                return WalkAction::Recurse;
            }
            if item.path.has_extension("md") {
                notes.push(item.clone());
            }
            WalkAction::Skip
        });
        notes.sort_by(|left, right| left.path.cmp(&right.path));
        notes
    }
}

fn store(error: impl ToString) -> Error {
    Error::Store(StoreError::Other(error.to_string()))
}

impl VaultReader for VaultBridge {
    fn search(&self, query: &str, mode: SearchMode, limit: usize) -> Result<Vec<SearchHit>, Error> {
        let mode = match mode {
            SearchMode::All => IndexMode::All,
            SearchMode::Any => IndexMode::Any,
            SearchMode::Phrase => IndexMode::Phrase,
        };
        let limit = u32::try_from(limit).unwrap_or(u32::MAX);
        let hits = self.search.search(query, mode, limit).map_err(store)?;
        Ok(hits
            .into_iter()
            .map(|hit| SearchHit {
                path: hit.path,
                snippet: hit.snippet,
            })
            .collect())
    }

    fn grep(
        &self,
        pattern: &Regex,
        folder: Option<&RelativePath>,
        max_file_bytes: u64,
        limit: usize,
    ) -> Result<Vec<LineMatch>, Error> {
        let root = folder.cloned().unwrap_or_default();
        let mut matches = Vec::new();
        for note in self.notes_under(&root) {
            if matches.len() >= limit {
                break;
            }
            let Ok(metadata) = self.files.metadata(&note.path) else {
                continue;
            };
            if metadata.size > max_file_bytes {
                continue;
            }
            let Ok(bytes) = self.files.read(&note.path) else {
                continue;
            };
            let body = String::from_utf8_lossy(&bytes);
            for (index, line) in body.lines().enumerate() {
                if matches.len() >= limit {
                    break;
                }
                if pattern.is_match(line) {
                    let text = line.trim().to_owned();
                    matches.push(LineMatch {
                        path: note.path.clone(),
                        line: index + 1,
                        text,
                    });
                }
            }
        }
        Ok(matches)
    }

    fn find_notes(&self, name: &str, limit: usize) -> Result<Vec<RelativePath>, Error> {
        let entries = self.listing.search_files(name);
        let notes = entries
            .into_iter()
            .filter(|entry| !entry.is_dir)
            .map(|entry| entry.path);
        Ok(notes.take(limit).collect())
    }

    fn read_note(&self, path: &RelativePath) -> Result<String, Error> {
        let bytes = self.files.read(path)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn tags(&self) -> Result<Vec<TagSummary>, Error> {
        let tags = self.search.tags().map_err(store)?;
        Ok(tags
            .into_iter()
            .map(|tag| TagSummary {
                tag: tag.tag,
                paths: tag.files,
            })
            .collect())
    }

    fn backlinks(&self, path: &RelativePath) -> Result<Vec<RelativePath>, Error> {
        let links = self.search.backlinks(path).map_err(store)?;
        Ok(links.into_iter().map(|link| link.path).collect())
    }

    fn recent_notes(&self, since: UnixMillis) -> Result<Vec<RecentNote>, Error> {
        let mut recent: Vec<RecentNote> = self
            .notes_under(&RelativePath::root())
            .into_iter()
            .filter(|note| note.modified >= since)
            .map(|note| RecentNote {
                path: note.path,
                modified: note.modified,
            })
            .collect();
        recent.sort_by(|left, right| right.modified.cmp(&left.modified));
        Ok(recent)
    }
}
