use std::sync::Arc;

use margin_kernel::{EntryKind, FileSystem, HistoryStore, RelativePath, StoreError};

use crate::MemoryFileSystem;

const HISTORY_ROOT: &str = ".margin/history";
const FREE_NAME_ATTEMPTS: u64 = 1000;

#[derive(Debug)]
pub struct MemoryHistoryStore {
    files: Arc<MemoryFileSystem>,
}

impl MemoryHistoryStore {
    #[must_use]
    pub fn new(files: Arc<MemoryFileSystem>) -> Self {
        Self { files }
    }
}

fn snapshots(note: &RelativePath) -> Result<RelativePath, StoreError> {
    RelativePath::new(HISTORY_ROOT)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        .join(note.as_str())
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

impl HistoryStore for MemoryHistoryStore {
    fn rename_note(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError> {
        let source = snapshots(from)?;
        let destination = snapshots(to)?;
        if !self.files.exists(&source) || source == destination {
            return Ok(());
        }
        if source.as_str().eq_ignore_ascii_case(destination.as_str()) {
            return self.files.rename(&source, &destination);
        }
        merge(&*self.files, &source, &destination)
    }

    fn detach_note(
        &self,
        note: &RelativePath,
        destination: &RelativePath,
    ) -> Result<(), StoreError> {
        let source = snapshots(note)?;
        if !self.files.exists(&source) {
            return Ok(());
        }
        self.files.rename(&source, destination)
    }

    fn attach_snapshots(
        &self,
        source: &RelativePath,
        note: &RelativePath,
    ) -> Result<(), StoreError> {
        merge(&*self.files, source, &snapshots(note)?)
    }
}

fn merge(
    files: &dyn FileSystem,
    source: &RelativePath,
    destination: &RelativePath,
) -> Result<(), StoreError> {
    files.create_dir_all(destination)?;
    for entry in files.list(source)? {
        let child = join(source, &entry.name)?;
        let target = join(destination, &entry.name)?;
        if entry.metadata.kind == EntryKind::Directory {
            merge(files, &child, &target)?;
            continue;
        }
        if identical(files, &child, &target) {
            files.remove_file(&child)?;
            continue;
        }
        let target = if files.exists(&target) {
            free_name(files, destination, &entry.name)?
        } else {
            target
        };
        files.rename(&child, &target)?;
    }
    let _source_removed = files.remove_dir_all(source);
    Ok(())
}

fn identical(files: &dyn FileSystem, child: &RelativePath, target: &RelativePath) -> bool {
    let Ok(existing) = files.read(target) else {
        return false;
    };
    files.read(child).is_ok_and(|moved| existing == moved)
}

fn free_name(
    files: &dyn FileSystem,
    directory: &RelativePath,
    name: &str,
) -> Result<RelativePath, StoreError> {
    let (stem, extension) = match name.rfind('.') {
        Some(index) if index > 0 => name.split_at(index),
        _ => (name, ""),
    };
    let numeric = stem.parse::<u64>().ok();
    let start = numeric.map_or(2, |timestamp| timestamp.saturating_add(1));
    let separator = if numeric.is_some() { "" } else { "-" };
    for number in start..start.saturating_add(FREE_NAME_ATTEMPTS) {
        let candidate = join(directory, &format!("{stem}{separator}{number}{extension}"))?;
        if !files.exists(&candidate) {
            return Ok(candidate);
        }
    }
    Err(StoreError::Other(format!(
        "no free snapshot name for {name}"
    )))
}

fn join(directory: &RelativePath, name: &str) -> Result<RelativePath, StoreError> {
    directory
        .join(name)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}
