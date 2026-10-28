use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use margin_kernel::{
    EntryKind, FileSystem, HistoryStore, HostFiles, HostWriter, Metadata, RelativePath, UnixMillis,
};
use margin_trash::TrashService;
use parking_lot::Mutex;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::domain::{
    FsEntry, MAX_WALK_DEPTH, SortBy, TreeRow, VaultIndex, WalkAction, join, listing,
    replace_in_text, visible_tree, walk,
};
use crate::error::Error;

pub const MAX_RESULTS: usize = 200;

#[derive(Debug, Default)]
struct SearchCache {
    index: Option<VaultIndex>,
    dirty: bool,
}

pub struct FilesService {
    files: Arc<dyn FileSystem>,
    host: Arc<dyn HostFiles>,
    history: Arc<dyn HistoryStore>,
    trash: Arc<TrashService>,
    vault_root: PathBuf,
    search: Mutex<SearchCache>,
}

impl fmt::Debug for FilesService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilesService").finish_non_exhaustive()
    }
}

impl FilesService {
    #[must_use]
    pub fn new(
        files: Arc<dyn FileSystem>,
        host: Arc<dyn HostFiles>,
        history: Arc<dyn HistoryStore>,
        trash: Arc<TrashService>,
        vault_root: PathBuf,
    ) -> Self {
        Self {
            files,
            host,
            history,
            trash,
            vault_root,
            search: Mutex::default(),
        }
    }

    #[must_use]
    pub fn list_directory(&self, directory: &RelativePath) -> Vec<FsEntry> {
        listing(&*self.files, directory)
    }

    #[must_use]
    pub fn walk_directory(&self, root: &RelativePath, include_hidden: bool) -> Vec<FsEntry> {
        let mut entries = Vec::new();
        walk(&*self.files, root, &mut |item| {
            if !include_hidden && item.name.starts_with('.') {
                return WalkAction::Skip;
            }
            entries.push(FsEntry {
                name: item.name.clone(),
                is_dir: item.is_dir,
                path: item.path.clone(),
                modified: item.modified,
            });
            WalkAction::Recurse
        });
        entries
    }

    #[must_use]
    pub fn visible_tree(
        &self,
        expanded: &[RelativePath],
        sort_by: SortBy,
        hidden: &[RelativePath],
    ) -> Vec<TreeRow> {
        self.tree_rows(&RelativePath::root(), 0, expanded, sort_by, hidden)
    }

    #[must_use]
    pub fn subtree(
        &self,
        folder: &RelativePath,
        depth_offset: usize,
        expanded: &[RelativePath],
        sort_by: SortBy,
        hidden: &[RelativePath],
    ) -> Vec<TreeRow> {
        self.tree_rows(folder, depth_offset, expanded, sort_by, hidden)
    }

    pub fn create_directory(&self, path: &RelativePath) -> Result<(), Error> {
        self.files.create_dir_all(path)?;
        self.invalidate_search_cache();
        Ok(())
    }

    pub fn rename_entry(&self, from: &RelativePath, to: &RelativePath) -> Result<(), Error> {
        if self.files.exists(to) && !self.same_entry(from, to) {
            return Err(occupied(to));
        }
        self.prepare_parent(to)?;
        self.files.rename(from, to)?;
        self.history.rename_note(from, to)?;
        self.invalidate_search_cache();
        Ok(())
    }

    pub fn delete_entry(&self, path: &RelativePath) -> Result<(), Error> {
        self.trash.move_entry(path)?;
        self.invalidate_search_cache();
        Ok(())
    }

    pub fn copy_file(&self, from: &RelativePath, to: &RelativePath) -> Result<(), Error> {
        self.require_free(to)?;
        self.prepare_parent(to)?;
        self.files.copy_file(from, to)?;
        self.invalidate_search_cache();
        Ok(())
    }

    pub fn copy_directory(&self, from: &RelativePath, to: &RelativePath) -> Result<(), Error> {
        if to.starts_with(from) {
            return Err(Error::NestedDestination);
        }
        self.require_free(to)?;
        self.files.create_dir_all(to)?;
        self.copy_vault_tree(from, to)?;
        self.invalidate_search_cache();
        Ok(())
    }

    pub fn import_file(&self, from: &Path, to: &RelativePath) -> Result<(), Error> {
        self.require_free(to)?;
        self.prepare_parent(to)?;
        let contents = self.host.read(from)?;
        self.files.write(to, &contents)?;
        self.invalidate_search_cache();
        Ok(())
    }

    pub fn import_directory(&self, from: &Path, to: &RelativePath) -> Result<(), Error> {
        if !self.host.metadata(from).is_ok_and(is_directory) {
            return Err(Error::NotADirectory(from.to_string_lossy().into_owned()));
        }
        if self.vault_root.starts_with(from) {
            return Err(Error::NestedDestination);
        }
        self.require_free(to)?;
        self.files.create_dir_all(to)?;
        self.copy_host_tree(from, to, 1)?;
        self.invalidate_search_cache();
        Ok(())
    }

    #[must_use]
    pub fn exists(&self, path: &RelativePath) -> bool {
        self.files.exists(path)
    }

    pub fn metadata(&self, path: &RelativePath) -> Result<Metadata, Error> {
        Ok(self.files.metadata(path)?)
    }

    pub fn set_modified(&self, path: &RelativePath, modified: UnixMillis) -> Result<(), Error> {
        self.files.set_modified(path, modified)?;
        Ok(())
    }

    pub fn read_file(&self, path: &RelativePath) -> Result<Vec<u8>, Error> {
        Ok(self.files.read(path)?)
    }

    pub fn write_file(&self, path: &RelativePath, contents: &[u8]) -> Result<(), Error> {
        self.files.write(path, contents)?;
        self.invalidate_search_cache();
        Ok(())
    }

    pub fn write_external_file(&self, path: &Path, contents: &[u8]) -> Result<(), Error> {
        self.host.write(path, contents)?;
        Ok(())
    }

    #[must_use]
    pub fn search_files(&self, query: &str) -> Vec<FsEntry> {
        let mut cache = self.search.lock();
        if cache.index.is_none() || cache.dirty {
            cache.index = Some(VaultIndex::build(&*self.files, &RelativePath::root()));
            cache.dirty = false;
        }
        cache
            .index
            .as_ref()
            .map_or_else(Vec::new, |index| index.search(query, MAX_RESULTS))
    }

    pub fn replace_in_file(
        &self,
        path: &RelativePath,
        search: &str,
        replace: &str,
        case_sensitive: bool,
    ) -> Result<u32, Error> {
        let contents = self.files.read(path)?;
        let text = String::from_utf8(contents).map_err(|_not_utf8| Error::NotText {
            path: path.to_string(),
        })?;
        let (replaced, count) = replace_in_text(&text, search, replace, case_sensitive);
        if count > 0 {
            self.files.write(path, replaced.as_bytes())?;
            self.invalidate_search_cache();
        }
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    }

    pub fn export_vault_zip(&self, destination: &Path) -> Result<(), Error> {
        let mut archive = ZipWriter::new(self.host.create(destination)?);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let mut failure: Option<Error> = None;
        walk(&*self.files, &RelativePath::root(), &mut |item| {
            if item.name.starts_with('.') || failure.is_some() {
                return WalkAction::Skip;
            }
            if item.is_dir {
                if let Err(error) = archive.add_directory(format!("{}/", item.path), options) {
                    failure = Some(error.into());
                    return WalkAction::Skip;
                }
                return WalkAction::Recurse;
            }
            match self.add_zip_entry(&mut archive, &item.path, options) {
                Ok(()) => {}
                Err(error) => failure = Some(error),
            }
            WalkAction::Skip
        });
        if let Some(failure) = failure {
            return Err(failure);
        }
        archive.finish().map_err(Error::from)?;
        Ok(())
    }

    pub fn invalidate_search_cache(&self) {
        self.search.lock().dirty = true;
    }

    fn tree_rows(
        &self,
        root: &RelativePath,
        depth: usize,
        expanded: &[RelativePath],
        sort_by: SortBy,
        hidden: &[RelativePath],
    ) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        visible_tree(
            &*self.files,
            root,
            depth,
            &expanded.iter().cloned().collect(),
            sort_by,
            &hidden.iter().cloned().collect(),
            &mut rows,
        );
        rows
    }

    fn add_zip_entry(
        &self,
        archive: &mut ZipWriter<Box<dyn HostWriter>>,
        path: &RelativePath,
        options: SimpleFileOptions,
    ) -> Result<(), Error> {
        archive.start_file(path.as_str(), options)?;
        let contents = self.files.read(path)?;
        archive.write_all(&contents)?;
        Ok(())
    }

    fn copy_vault_tree(&self, from: &RelativePath, to: &RelativePath) -> Result<(), Error> {
        let mut pending: Vec<(RelativePath, RelativePath)> = Vec::new();
        let mut failure: Option<Error> = None;
        walk(&*self.files, from, &mut |item| {
            if failure.is_some() || item.is_symlink {
                return WalkAction::Skip;
            }
            let Some(relative) = item.path.strip_prefix(from) else {
                failure = Some(Error::UnresolvedDestination);
                return WalkAction::Skip;
            };
            if relative.components().count() > MAX_WALK_DEPTH {
                failure = Some(Error::TooDeep);
                return WalkAction::Skip;
            }
            let target = join(to, relative.as_str());
            if item.is_dir {
                if let Err(error) = self.files.create_dir_all(&target) {
                    failure = Some(error.into());
                }
                return WalkAction::Recurse;
            }
            pending.push((item.path.clone(), target));
            WalkAction::Skip
        });
        if let Some(failure) = failure {
            return Err(failure);
        }
        for (source, target) in pending {
            self.files.copy_file(&source, &target)?;
        }
        Ok(())
    }

    fn copy_host_tree(
        &self,
        source: &Path,
        destination: &RelativePath,
        depth: usize,
    ) -> Result<(), Error> {
        if depth > MAX_WALK_DEPTH {
            return Err(Error::TooDeep);
        }
        for entry in self.host.list(source)? {
            if entry.metadata.kind == EntryKind::Symlink {
                continue;
            }
            let child = source.join(&entry.name);
            let target = join(destination, &entry.name);
            if entry.metadata.kind == EntryKind::Directory {
                self.files.create_dir_all(&target)?;
                self.copy_host_tree(&child, &target, depth + 1)?;
                continue;
            }
            let contents = self.host.read(&child)?;
            self.files.write(&target, &contents)?;
        }
        Ok(())
    }

    fn same_entry(&self, from: &RelativePath, to: &RelativePath) -> bool {
        if from.parent() != to.parent() {
            return false;
        }
        if from.file_name() == to.file_name() {
            return true;
        }
        if from.file_name().to_lowercase() != to.file_name().to_lowercase() {
            return false;
        }
        self.files
            .list(&from.parent().unwrap_or_default())
            .is_ok_and(|entries| !entries.iter().any(|entry| entry.name == to.file_name()))
    }

    fn require_free(&self, path: &RelativePath) -> Result<(), Error> {
        if self.files.exists(path) {
            return Err(occupied(path));
        }
        Ok(())
    }

    fn prepare_parent(&self, path: &RelativePath) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            self.files.create_dir_all(&parent)?;
        }
        Ok(())
    }
}

fn occupied(path: &RelativePath) -> Error {
    let name = if path.is_root() {
        path.to_string()
    } else {
        path.file_name().to_owned()
    };
    Error::Occupied { name }
}

fn is_directory(metadata: Metadata) -> bool {
    metadata.kind == EntryKind::Directory
}
