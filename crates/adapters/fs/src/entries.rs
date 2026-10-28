use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use margin_kernel::{DirEntry, EntryKind, Metadata, StoreError, UnixMillis};

use crate::io_error::from_io;

pub(crate) fn metadata_of(path: &Path) -> Result<Metadata, StoreError> {
    fs::symlink_metadata(path)
        .map(|metadata| describe(&metadata))
        .map_err(|error| from_io(&error, path))
}

pub(crate) fn entries_of(directory: &Path) -> Result<Vec<DirEntry>, StoreError> {
    let read = fs::read_dir(directory).map_err(|error| from_io(&error, directory))?;
    let mut entries = Vec::new();
    for entry in read {
        let entry = entry.map_err(|error| from_io(&error, directory))?;
        let metadata = metadata_of(&entry.path())?;
        entries.push(DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            metadata,
        });
    }
    Ok(entries)
}

pub(crate) fn describe(metadata: &fs::Metadata) -> Metadata {
    Metadata {
        kind: entry_kind(metadata.file_type()),
        size: metadata.len(),
        modified: modified_of(metadata),
    }
}

fn entry_kind(file_type: fs::FileType) -> EntryKind {
    if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_dir() {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}

fn modified_of(metadata: &fs::Metadata) -> UnixMillis {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or_else(UnixMillis::default, |elapsed| {
            UnixMillis::from_millis(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        })
}
