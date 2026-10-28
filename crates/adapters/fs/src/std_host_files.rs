use std::fs;
use std::path::Path;

use margin_kernel::{DirEntry, HostFiles, HostWriter, Metadata, StoreError};

use crate::entries::{describe, entries_of};
use crate::io_error::from_io;

#[derive(Clone, Copy, Debug, Default)]
pub struct StdHostFiles;

impl StdHostFiles {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl HostFiles for StdHostFiles {
    fn read(&self, path: &Path) -> Result<Vec<u8>, StoreError> {
        if fs::metadata(path)
            .map_err(|error| from_io(&error, path))?
            .is_dir()
        {
            return Err(StoreError::InvalidData(format!(
                "{} is a directory",
                path.display()
            )));
        }
        fs::read(path).map_err(|error| from_io(&error, path))
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), StoreError> {
        create_parents(path)?;
        fs::write(path, contents).map_err(|error| from_io(&error, path))
    }

    fn create(&self, path: &Path) -> Result<Box<dyn HostWriter>, StoreError> {
        create_parents(path)?;
        let file = fs::File::create(path).map_err(|error| from_io(&error, path))?;
        Ok(Box::new(file))
    }

    fn metadata(&self, path: &Path) -> Result<Metadata, StoreError> {
        fs::metadata(path)
            .map(|metadata| describe(&metadata))
            .map_err(|error| from_io(&error, path))
    }

    fn list(&self, directory: &Path) -> Result<Vec<DirEntry>, StoreError> {
        match fs::symlink_metadata(directory) {
            Ok(metadata) if metadata.is_dir() => entries_of(directory),
            Ok(_) => Err(StoreError::InvalidData(format!(
                "{} is not a directory",
                directory.display()
            ))),
            Err(error) => Err(from_io(&error, directory)),
        }
    }
}

fn create_parents(path: &Path) -> Result<(), StoreError> {
    ensure_ancestors_are_directories(path)?;
    match path.parent() {
        Some(parent) => fs::create_dir_all(parent).map_err(|error| from_io(&error, path)),
        None => Ok(()),
    }
}

fn ensure_ancestors_are_directories(path: &Path) -> Result<(), StoreError> {
    for ancestor in path.ancestors().skip(1) {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => return Ok(()),
            Ok(_) => return Err(StoreError::AlreadyExists(ancestor.display().to_string())),
            Err(_) => {}
        }
    }
    Ok(())
}
