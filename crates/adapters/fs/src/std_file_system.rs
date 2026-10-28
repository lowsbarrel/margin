use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use filetime::{FileTime, set_file_mtime};
use margin_kernel::{
    DirEntry, EntryKind, FileSystem, Metadata, RelativePath, StoreError, UnixMillis,
};

use crate::atomic_write::{Privacy, atomic_write};
use crate::entries::{entries_of, metadata_of};
use crate::io_error::from_io;

#[derive(Debug)]
pub struct StdFileSystem {
    root: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FinalComponent {
    Canonicalize,
    AsWritten,
}

impl StdFileSystem {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn resolve(&self, path: &RelativePath) -> Result<PathBuf, StoreError> {
        resolve_within(&self.root, path, FinalComponent::Canonicalize)
    }

    fn resolve_as_written(&self, path: &RelativePath) -> Result<PathBuf, StoreError> {
        resolve_within(&self.root, path, FinalComponent::AsWritten)
    }

    fn store(
        &self,
        path: &RelativePath,
        contents: &[u8],
        privacy: Privacy,
    ) -> Result<(), StoreError> {
        if path.is_root() {
            return Err(StoreError::InvalidData(String::from(
                "the root is a directory",
            )));
        }
        let destination = self.resolve(path)?;
        if fs::symlink_metadata(&destination).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(StoreError::AlreadyExists(path.to_string()));
        }
        create_parent_directories(&destination)?;
        atomic_write(&destination, contents, privacy)
    }
}

fn resolve_within(
    root: &Path,
    path: &RelativePath,
    final_component: FinalComponent,
) -> Result<PathBuf, StoreError> {
    if final_component == FinalComponent::AsWritten && !path.is_root() {
        let parent = path.parent().unwrap_or_default();
        let resolved =
            resolve_within(root, &parent, FinalComponent::Canonicalize)?.join(path.file_name());
        return Ok(resolved);
    }
    let canonical_root = root.canonicalize().map_err(|error| {
        StoreError::Unavailable(format!("cannot resolve {}: {error}", root.display()))
    })?;
    let resolved = deepest_existing_ancestor(&whole_path(root, path))?;
    if !resolved.starts_with(&canonical_root) {
        return Err(StoreError::OutsideRoot(path.to_string()));
    }
    Ok(resolved)
}

fn whole_path(root: &Path, path: &RelativePath) -> PathBuf {
    if path.is_root() {
        root.to_path_buf()
    } else {
        root.join(path.as_str())
    }
}

fn deepest_existing_ancestor(target: &Path) -> Result<PathBuf, StoreError> {
    let mut missing: Vec<&OsStr> = Vec::new();
    let mut ancestor = target;
    let mut resolved = loop {
        if let Ok(canonical) = ancestor.canonicalize() {
            break canonical;
        }
        match ancestor.parent() {
            Some(parent) => {
                if let Some(name) = ancestor.file_name() {
                    missing.push(name);
                }
                ancestor = parent;
            }
            None => return Err(StoreError::OutsideRoot(target.display().to_string())),
        }
    };
    for name in missing.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

fn destination_conflicts(source: &Path, destination: &Path) -> bool {
    fs::symlink_metadata(destination).is_ok() && !is_the_same_entry(source, destination)
}

fn is_the_same_entry(source: &Path, destination: &Path) -> bool {
    if source.parent() != destination.parent() {
        return false;
    }
    let (Some(source_name), Some(destination_name)) = (source.file_name(), destination.file_name())
    else {
        return false;
    };
    if source_name == destination_name {
        return true;
    }
    if !source_name
        .to_string_lossy()
        .eq_ignore_ascii_case(&destination_name.to_string_lossy())
    {
        return false;
    }
    match source.parent() {
        Some(parent) => fs::read_dir(parent).is_ok_and(|entries| {
            !entries
                .flatten()
                .any(|entry| entry.file_name().as_os_str() == destination_name)
        }),
        None => false,
    }
}

fn create_parent_directories(destination: &Path) -> Result<(), StoreError> {
    ensure_ancestors_are_directories(destination)?;
    match destination.parent() {
        Some(parent) => fs::create_dir_all(parent).map_err(|error| from_io(&error, destination)),
        None => Ok(()),
    }
}

fn ensure_ancestors_are_directories(destination: &Path) -> Result<(), StoreError> {
    for ancestor in destination.ancestors().skip(1) {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => return Ok(()),
            Ok(_) => return Err(StoreError::AlreadyExists(ancestor.display().to_string())),
            Err(_) => {}
        }
    }
    Ok(())
}

impl FileSystem for StdFileSystem {
    fn read(&self, path: &RelativePath) -> Result<Vec<u8>, StoreError> {
        let source = self.resolve(path)?;
        if metadata_of(&source)?.kind != EntryKind::File {
            return Err(StoreError::InvalidData(format!("{path} is not a file")));
        }
        fs::read(&source).map_err(|error| from_io(&error, &source))
    }

    fn write(&self, path: &RelativePath, contents: &[u8]) -> Result<(), StoreError> {
        self.store(path, contents, Privacy::Inherited)
    }

    fn write_private(&self, path: &RelativePath, contents: &[u8]) -> Result<(), StoreError> {
        self.store(path, contents, Privacy::OwnerOnly)
    }

    fn metadata(&self, path: &RelativePath) -> Result<Metadata, StoreError> {
        metadata_of(&self.resolve(path)?)
    }

    fn list(&self, directory: &RelativePath) -> Result<Vec<DirEntry>, StoreError> {
        let resolved = self.resolve(directory)?;
        match fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.is_dir() => entries_of(&resolved),
            Ok(_) => Err(StoreError::InvalidData(format!(
                "{directory} is not a directory"
            ))),
            Err(error) => Err(from_io(&error, &resolved)),
        }
    }

    fn create_dir_all(&self, path: &RelativePath) -> Result<(), StoreError> {
        if path.is_root() {
            return Ok(());
        }
        let resolved = self.resolve(path)?;
        match fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.is_dir() => Ok(()),
            Ok(_) => Err(StoreError::AlreadyExists(path.to_string())),
            Err(_) => {
                ensure_ancestors_are_directories(&resolved)?;
                fs::create_dir_all(&resolved).map_err(|error| from_io(&error, &resolved))
            }
        }
    }

    fn rename(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError> {
        let source = self.resolve(from)?;
        let destination = self.resolve_as_written(to)?;
        fs::symlink_metadata(&source).map_err(|error| from_io(&error, &source))?;
        if from == to {
            return Ok(());
        }
        if to.starts_with(from) {
            return Err(StoreError::InvalidData(format!(
                "cannot move {from} into itself"
            )));
        }
        if destination_conflicts(&source, &destination) {
            return Err(StoreError::AlreadyExists(to.to_string()));
        }
        create_parent_directories(&destination)?;
        fs::rename(&source, &destination).map_err(|error| from_io(&error, &destination))
    }

    fn copy_file(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError> {
        let source = self.resolve(from)?;
        let destination = self.resolve(to)?;
        if !fs::metadata(&source)
            .map_err(|error| from_io(&error, &source))?
            .is_file()
        {
            return Err(StoreError::InvalidData(format!("{from} is not a file")));
        }
        if fs::symlink_metadata(&destination).is_ok() {
            return Err(StoreError::AlreadyExists(to.to_string()));
        }
        create_parent_directories(&destination)?;
        fs::copy(&source, &destination).map_err(|error| from_io(&error, &destination))?;
        Ok(())
    }

    fn remove_file(&self, path: &RelativePath) -> Result<(), StoreError> {
        let resolved = self.resolve(path)?;
        match fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.is_dir() => {
                Err(StoreError::InvalidData(format!("{path} is a directory")))
            }
            Ok(_) => fs::remove_file(&resolved).map_err(|error| from_io(&error, &resolved)),
            Err(error) => Err(from_io(&error, &resolved)),
        }
    }

    fn remove_dir_all(&self, path: &RelativePath) -> Result<(), StoreError> {
        if path.is_root() {
            return Err(StoreError::InvalidData(String::from(
                "the root cannot be removed",
            )));
        }
        let resolved = self.resolve(path)?;
        match fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.is_dir() => {
                fs::remove_dir_all(&resolved).map_err(|error| from_io(&error, &resolved))
            }
            Ok(_) => Err(StoreError::InvalidData(format!(
                "{path} is not a directory"
            ))),
            Err(error) => Err(from_io(&error, &resolved)),
        }
    }

    fn set_modified(&self, path: &RelativePath, modified: UnixMillis) -> Result<(), StoreError> {
        let resolved = self.resolve(path)?;
        fs::symlink_metadata(&resolved).map_err(|error| from_io(&error, &resolved))?;
        let seconds = i64::try_from(modified.as_seconds()).unwrap_or(i64::MAX);
        let nanoseconds =
            u32::try_from((modified.as_millis() % 1000) * 1_000_000).unwrap_or_default();
        set_file_mtime(&resolved, FileTime::from_unix_time(seconds, nanoseconds))
            .map_err(|error| from_io(&error, &resolved))
    }
}
