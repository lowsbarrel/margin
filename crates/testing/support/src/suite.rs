use std::sync::Arc;

use margin_kernel::{DirEntry, EntryKind, FileSystem, RelativePath, StoreError};

pub(crate) type Factory<'factory> = &'factory dyn Fn() -> Arc<dyn FileSystem>;

pub(crate) fn path(raw: &str) -> RelativePath {
    match RelativePath::new(raw) {
        Ok(path) => path,
        Err(error) => panic!("the suite path `{raw}` is invalid: {error}"),
    }
}

pub(crate) fn ok<T>(result: Result<T, StoreError>, action: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{action} failed: {error}"),
    }
}

pub(crate) fn failed<T>(result: Result<T, StoreError>, action: &str) -> StoreError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("{action} should have failed"),
    }
}

pub(crate) fn names(entries: &[DirEntry]) -> Vec<String> {
    let mut names: Vec<String> = entries.iter().map(|entry| entry.name.clone()).collect();
    names.sort_unstable();
    names
}

pub(crate) fn kind_of(entries: &[DirEntry], name: &str) -> EntryKind {
    match entries.iter().find(|entry| entry.name == name) {
        Some(entry) => entry.metadata.kind,
        None => panic!("`{name}` is missing from the listing"),
    }
}

pub(crate) fn size_of(entries: &[DirEntry], name: &str) -> u64 {
    match entries.iter().find(|entry| entry.name == name) {
        Some(entry) => entry.metadata.size,
        None => panic!("`{name}` is missing from the listing"),
    }
}
