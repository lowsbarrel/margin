use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use margin_files::{WalkAction, walk};
use margin_kernel::{Clock, EntryKind, FileSystem, HostFiles, RelativePath};
use margin_trash::TrashService;

use crate::domain::{MAX_ATTEMPTS, attachment_name, is_stored_name, referenced_names, suffixed};
use crate::error::Error;

pub struct AttachmentsService {
    files: Arc<dyn FileSystem>,
    host: Arc<dyn HostFiles>,
    trash: Arc<TrashService>,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for AttachmentsService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentsService").finish_non_exhaustive()
    }
}

impl AttachmentsService {
    #[must_use]
    pub fn new(
        files: Arc<dyn FileSystem>,
        host: Arc<dyn HostFiles>,
        trash: Arc<TrashService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            files,
            host,
            trash,
            clock,
        }
    }

    pub fn store(
        &self,
        folder: &RelativePath,
        original_name: &str,
        bytes: &[u8],
    ) -> Result<RelativePath, Error> {
        let folder = attachment_dir(folder)?;
        let name = attachment_name(original_name, bytes);
        for attempt in 0..MAX_ATTEMPTS {
            let candidate = if attempt == 0 {
                name.clone()
            } else {
                suffixed(&name, attempt + 1)
            };
            let path = folder
                .join(&candidate)
                .map_err(|_invalid| Error::NoFreeName)?;
            if self.files.exists(&path) {
                if self
                    .files
                    .read(&path)
                    .is_ok_and(|existing| existing == bytes)
                {
                    return Ok(path);
                }
                continue;
            }
            self.files.write(&path, bytes)?;
            return Ok(path);
        }
        Err(Error::NoFreeName)
    }

    pub fn import_file(&self, from: &Path, folder: &RelativePath) -> Result<RelativePath, Error> {
        let bytes = self.host.read(from)?;
        let name = from
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        self.store(folder, &name, &bytes)
    }

    pub fn unused_files(&self, folder: &RelativePath) -> Result<Vec<RelativePath>, Error> {
        let folder = attachment_dir(folder)?;
        if !self.is_directory(&folder) {
            return Ok(Vec::new());
        }
        let mut notes: Vec<RelativePath> = Vec::new();
        walk(&*self.files, &RelativePath::root(), &mut |item| {
            if item.name.starts_with('.') {
                return WalkAction::Skip;
            }
            if item.is_dir {
                return WalkAction::Recurse;
            }
            if item.path.has_extension("md") {
                notes.push(item.path.clone());
            }
            WalkAction::Skip
        });

        let mut referenced: HashSet<String> = HashSet::new();
        for note in notes {
            if let Ok(bytes) = self.files.read(&note)
                && let Ok(markdown) = String::from_utf8(bytes)
            {
                referenced_names(&markdown, &mut referenced);
            }
        }

        let mut unused: Vec<RelativePath> = Vec::new();
        walk(&*self.files, &folder, &mut |item| {
            if item.name.starts_with('.') {
                return WalkAction::Skip;
            }
            if item.is_dir {
                return WalkAction::Recurse;
            }
            if !referenced.contains(&item.name) {
                unused.push(item.path.clone());
            }
            WalkAction::Skip
        });
        unused.sort();
        Ok(unused)
    }

    pub fn sweep_unused(&self, folder: &RelativePath, grace: Duration) -> Result<u32, Error> {
        let now = self.clock.now();
        let mut swept = 0_u32;
        for path in self.unused_files(folder)? {
            let stored = is_stored_name(path.file_name());
            let stale = self
                .files
                .metadata(&path)
                .is_ok_and(|metadata| now.elapsed_since(metadata.modified) >= grace);
            if !stored || !stale {
                continue;
            }
            self.trash.move_entry(&path)?;
            swept += 1;
        }
        Ok(swept)
    }

    fn is_directory(&self, path: &RelativePath) -> bool {
        self.files
            .metadata(path)
            .is_ok_and(|metadata| metadata.kind == EntryKind::Directory)
    }
}

fn attachment_dir(folder: &RelativePath) -> Result<RelativePath, Error> {
    if folder.is_root() || folder.is_hidden() {
        return Err(Error::InvalidFolder(folder.to_string()));
    }
    Ok(folder.clone())
}
