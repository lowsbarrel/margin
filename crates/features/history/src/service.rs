use std::cmp::Reverse;
use std::fmt;
use std::sync::Arc;

use margin_kernel::{Clock, EntryKind, FileSystem, HistoryStore, RelativePath, StoreError};

use crate::domain::{self, Snapshot};
use crate::error::Error;

const HISTORY_ROOT: &str = ".margin/history";

#[derive(Clone)]
pub struct History {
    files: Arc<dyn FileSystem>,
    clock: Arc<dyn Clock>,
}

impl History {
    #[must_use]
    pub fn new(files: Arc<dyn FileSystem>, clock: Arc<dyn Clock>) -> Self {
        Self { files, clock }
    }

    pub fn save(&self, file: &RelativePath, content: &[u8]) -> Result<String, Error> {
        let directory = history_dir(file)?;
        self.files.create_dir_all(&directory)?;

        let existing = self.snapshot_files(&directory);
        if let Some((_, newest)) = existing.iter().max_by_key(|(timestamp, _)| *timestamp)
            && let Ok(bytes) = self.files.read(&directory.join(newest)?)
            && bytes == content
        {
            self.prune(&directory);
            return Ok(newest.clone());
        }

        let extension = file.extension().unwrap_or("md");
        let mut timestamp = self.clock.now().as_millis();
        let mut name = format!("{timestamp}.{extension}");
        while self.files.exists(&directory.join(&name)?) {
            timestamp += 1;
            name = format!("{timestamp}.{extension}");
        }
        self.files.write(&directory.join(&name)?, content)?;
        self.prune(&directory);
        Ok(name)
    }

    pub fn list(&self, file: &RelativePath) -> Result<Vec<Snapshot>, Error> {
        let directory = history_dir(file)?;
        if !self.is_dir(&directory) {
            return Ok(Vec::new());
        }
        let mut snapshots: Vec<(u64, Snapshot)> = self
            .files
            .list(&directory)?
            .into_iter()
            .filter(|entry| entry.metadata.kind == EntryKind::File)
            .filter_map(|entry| {
                let millis = domain::snapshot_millis(&entry.name)?;
                let snapshot = Snapshot {
                    filename: entry.name,
                    timestamp: millis / 1000,
                    size: entry.metadata.size,
                };
                Some((millis, snapshot))
            })
            .collect();
        snapshots.sort_by_key(|(millis, _)| Reverse(*millis));
        Ok(snapshots
            .into_iter()
            .map(|(_, snapshot)| snapshot)
            .collect())
    }

    pub fn read(&self, file: &RelativePath, snapshot: &str) -> Result<Vec<u8>, Error> {
        Ok(self.files.read(&snapshot_path(file, snapshot)?)?)
    }

    pub fn delete(&self, file: &RelativePath, snapshot: &str) -> Result<(), Error> {
        self.files.remove_file(&snapshot_path(file, snapshot)?)?;
        Ok(())
    }

    pub fn clear(&self, file: &RelativePath) -> Result<u32, Error> {
        let directory = history_dir(file)?;
        if !self.is_dir(&directory) {
            return Ok(0);
        }
        let mut count = 0_u32;
        for entry in self.files.list(&directory)? {
            if entry.metadata.kind == EntryKind::File {
                self.files.remove_file(&directory.join(&entry.name)?)?;
                count += 1;
            }
        }
        self.remove_dir_if_empty(&directory);
        Ok(count)
    }

    pub fn rename(&self, from: &RelativePath, to: &RelativePath) -> Result<(), Error> {
        let source = history_dir(from)?;
        let destination = history_dir(to)?;
        if !self.files.exists(&source) {
            return Ok(());
        }
        if from.as_str().to_lowercase() == to.as_str().to_lowercase() {
            if source == destination {
                return Ok(());
            }
            self.files.rename(&source, &destination)?;
            return Ok(());
        }
        self.merge_dirs(&source, &destination)
    }

    #[must_use]
    pub fn prune_orphans(&self) -> u32 {
        let root =
            RelativePath::new(HISTORY_ROOT).expect("the history root is a valid relative path");
        if !self.is_dir(&root) {
            return 0;
        }
        let mut removed = 0_u32;
        self.prune_orphan_dir(&root, &RelativePath::root(), &mut removed);
        removed
    }

    fn snapshot_files(&self, directory: &RelativePath) -> Vec<(u64, String)> {
        let Ok(entries) = self.files.list(directory) else {
            return Vec::new();
        };
        entries
            .into_iter()
            .filter(|entry| entry.metadata.kind == EntryKind::File)
            .filter_map(|entry| {
                domain::snapshot_millis(&entry.name).map(|stamp| (stamp, entry.name))
            })
            .collect()
    }

    fn prune(&self, directory: &RelativePath) {
        let files = self.snapshot_files(directory);
        let stamps: Vec<u64> = files.iter().map(|(stamp, _)| *stamp).collect();
        let kept = domain::retained_indices(&stamps, self.clock.now().as_millis());
        for (index, (_, name)) in files.iter().enumerate() {
            if kept.contains(&index) {
                continue;
            }
            if let Ok(path) = directory.join(name) {
                self.remove_file(&path);
            }
        }
    }

    fn remove_file(&self, path: &RelativePath) -> bool {
        self.files.remove_file(path).is_ok()
    }

    fn remove_dir_if_empty(&self, path: &RelativePath) -> bool {
        if self
            .files
            .list(path)
            .is_ok_and(|entries| entries.is_empty())
        {
            return self.files.remove_dir_all(path).is_ok();
        }
        false
    }

    fn is_dir(&self, path: &RelativePath) -> bool {
        self.files
            .metadata(path)
            .is_ok_and(|metadata| metadata.kind == EntryKind::Directory)
    }

    fn is_file(&self, path: &RelativePath) -> bool {
        self.files
            .metadata(path)
            .is_ok_and(|metadata| metadata.kind == EntryKind::File)
    }

    fn merge_dirs(&self, source: &RelativePath, target: &RelativePath) -> Result<(), Error> {
        self.files.create_dir_all(target)?;
        for entry in self.files.list(source)? {
            let child = source.join(&entry.name)?;
            let destination = target.join(&entry.name)?;
            if entry.metadata.kind == EntryKind::Directory {
                self.merge_dirs(&child, &destination)?;
                continue;
            }
            if let Ok(existing) = self.files.read(&destination)
                && existing == self.files.read(&child).unwrap_or_default()
            {
                self.remove_file(&child);
                continue;
            }
            let destination = if self.files.exists(&destination) {
                self.free_snapshot_path(target, &entry.name)?
            } else {
                destination
            };
            self.files.rename(&child, &destination)?;
        }
        self.remove_dir_if_empty(source);
        Ok(())
    }

    fn free_snapshot_path(
        &self,
        directory: &RelativePath,
        name: &str,
    ) -> Result<RelativePath, Error> {
        let (stem, extension) = match name.rfind('.') {
            Some(index) if index > 0 => (&name[..index], &name[index..]),
            _ => (name, ""),
        };
        if let Ok(mut timestamp) = stem.parse::<u64>() {
            loop {
                timestamp += 1;
                let candidate = directory.join(&format!("{timestamp}{extension}"))?;
                if !self.files.exists(&candidate) {
                    return Ok(candidate);
                }
            }
        }
        for index in 2..1000_u32 {
            let candidate = directory.join(&format!("{stem}-{index}{extension}"))?;
            if !self.files.exists(&candidate) {
                return Ok(candidate);
            }
        }
        Err(Error::NameExhausted(name.to_owned()))
    }

    fn prune_orphan_dir(
        &self,
        directory: &RelativePath,
        vault_file: &RelativePath,
        removed: &mut u32,
    ) -> bool {
        let Ok(entries) = self.files.list(directory) else {
            return false;
        };
        let mut kept_children = 0_u32;
        let mut kept_files = 0_u32;
        for entry in entries {
            let Ok(child) = directory.join(&entry.name) else {
                continue;
            };
            if entry.metadata.kind == EntryKind::Directory {
                let Ok(vault_child) = vault_file.join(&entry.name) else {
                    continue;
                };
                if self.prune_orphan_dir(&child, &vault_child, removed) {
                    kept_children += 1;
                }
            } else {
                kept_files += 1;
            }
        }
        let keep = kept_children > 0 || (kept_files > 0 && self.is_file(vault_file));
        if !keep && self.files.remove_dir_all(directory).is_ok() {
            *removed += 1;
        }
        keep
    }
}

impl fmt::Debug for History {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("History").finish_non_exhaustive()
    }
}

impl HistoryStore for History {
    fn rename_note(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError> {
        self.rename(from, to).map_err(Error::into_store_error)
    }

    fn detach_note(
        &self,
        note: &RelativePath,
        destination: &RelativePath,
    ) -> Result<(), StoreError> {
        let source = history_dir(note).map_err(Error::into_store_error)?;
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
        let destination = history_dir(note).map_err(Error::into_store_error)?;
        self.merge_dirs(source, &destination)
            .map_err(Error::into_store_error)
    }
}

fn history_dir(file: &RelativePath) -> Result<RelativePath, Error> {
    Ok(RelativePath::new(HISTORY_ROOT)?.join(file.as_str())?)
}

fn snapshot_path(file: &RelativePath, snapshot: &str) -> Result<RelativePath, Error> {
    if snapshot.contains('/') || snapshot.contains('\\') || snapshot.contains("..") {
        return Err(Error::InvalidSnapshotFilename(snapshot.to_owned()));
    }
    Ok(history_dir(file)?.join(snapshot)?)
}
