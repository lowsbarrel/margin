use std::time::Duration;

use margin_kernel::{EntryKind, FileSystem, RelativePath, UnixMillis};

use crate::error::Error;

pub const PURGE_AGE: Duration = Duration::from_hours(720);
pub(crate) const TRASH_ROOT: &str = ".margin/trash";
pub(crate) const FILES_DIR: &str = "files";
pub(crate) const HISTORY_DIR: &str = "history";
pub(crate) const REL_PATH_FILE: &str = "path";

const MAX_ITEM_DEPTH: usize = 10;
const FREE_NAME_LIMIT: u64 = 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashItem {
    pub id: String,
    pub name: String,
    pub path: RelativePath,
    pub is_dir: bool,
    pub deleted_at: UnixMillis,
    pub has_history: bool,
}

#[must_use]
pub(crate) fn timestamp_millis(id: &str) -> Option<UnixMillis> {
    id.split('-')
        .next()?
        .parse::<u64>()
        .ok()
        .map(UnixMillis::from_millis)
}

#[must_use]
pub(crate) fn join(directory: &RelativePath, relative: &str) -> RelativePath {
    directory.join(relative).expect("a relative path joins")
}

pub(crate) fn item_rel(files: &dyn FileSystem, item: &RelativePath) -> Result<RelativePath, Error> {
    if let Ok(recorded) = files.read(&join(item, REL_PATH_FILE))
        && let Ok(text) = String::from_utf8(recorded)
        && !text.is_empty()
    {
        let recorded = RelativePath::new(&text)
            .ok()
            .filter(|recorded| !recorded.is_root());
        return recorded.ok_or(Error::InvalidRecordedPath);
    }
    fallback_rel(files, item)
}

fn fallback_rel(files: &dyn FileSystem, item: &RelativePath) -> Result<RelativePath, Error> {
    let mut directory = join(item, FILES_DIR);
    let mut rel = RelativePath::root();
    for _ in 0..MAX_ITEM_DEPTH {
        let entries = files.list(&directory)?;
        let Some(first) = entries.first() else {
            return Err(Error::EmptyItem);
        };
        if entries.len() > 1 {
            break;
        }
        rel = join(&rel, &first.name);
        let next = join(&directory, &first.name);
        if first.metadata.kind != EntryKind::Directory {
            return Ok(rel);
        }
        directory = next;
    }
    if rel.is_root() {
        return Err(Error::UnrecordedPath);
    }
    Ok(rel)
}

#[must_use]
pub(crate) fn unique_restore_path(files: &dyn FileSystem, rel: &RelativePath) -> RelativePath {
    let parent = rel.parent().unwrap_or_default();
    let Ok(entries) = files.list(&parent) else {
        return rel.clone();
    };
    let taken: Vec<String> = entries
        .iter()
        .map(|entry| entry.name.to_lowercase())
        .collect();
    let name = rel.file_name();
    if !taken.contains(&name.to_lowercase()) {
        return rel.clone();
    }
    let (stem, extension) = match name.rfind('.') {
        Some(index) if index > 0 => name.split_at(index),
        _ => (name, ""),
    };
    for number in 1..FREE_NAME_LIMIT {
        let suffix = if number == 1 {
            String::from(" (restored)")
        } else {
            format!(" (restored {number})")
        };
        let candidate = format!("{stem}{suffix}{extension}");
        if !taken.contains(&candidate.to_lowercase()) {
            return rel
                .with_file_name(&candidate)
                .unwrap_or_else(|_| rel.clone());
        }
    }
    rel.clone()
}
