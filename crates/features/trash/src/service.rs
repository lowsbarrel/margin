use std::cmp::Reverse;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use margin_kernel::{Clock, EntryKind, FileSystem, HistoryStore, Page, PageRequest, RelativePath};

use crate::domain::{
    FILES_DIR, HISTORY_DIR, PURGE_AGE, REL_PATH_FILE, TRASH_ROOT, TrashItem, item_rel, join,
    timestamp_millis, unique_restore_path,
};
use crate::error::Error;

pub struct TrashService {
    files: Arc<dyn FileSystem>,
    history: Arc<dyn HistoryStore>,
    clock: Arc<dyn Clock>,
    counter: AtomicU64,
}

impl fmt::Debug for TrashService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrashService").finish_non_exhaustive()
    }
}

impl TrashService {
    #[must_use]
    pub fn new(
        files: Arc<dyn FileSystem>,
        history: Arc<dyn HistoryStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            files,
            history,
            clock,
            counter: AtomicU64::new(0),
        }
    }

    pub fn move_entry(&self, path: &RelativePath) -> Result<(), Error> {
        if path.is_root() {
            return Err(Error::VaultRoot);
        }
        let item = self.new_item();
        move_path(
            &*self.files,
            path,
            &join(&join(&item, FILES_DIR), path.as_str()),
        )?;
        self.files
            .write(&join(&item, REL_PATH_FILE), path.as_str().as_bytes())?;
        self.history
            .detach_note(path, &join(&join(&item, HISTORY_DIR), path.as_str()))?;
        Ok(())
    }

    #[must_use]
    pub fn list(&self, request: &PageRequest) -> Page<TrashItem> {
        let Ok(entries) = self.files.list(&trash_root()) else {
            return Page::from_ordered(Vec::new(), request);
        };
        let mut items: Vec<TrashItem> = entries
            .iter()
            .filter_map(|entry| self.describe(&entry.name))
            .collect();
        items.sort_by_key(|item| Reverse(item.deleted_at));
        Page::from_ordered(items, request)
    }

    pub fn restore(&self, id: &str) -> Result<RelativePath, Error> {
        let item = self.item_dir(id)?;
        let rel = item_rel(&*self.files, &item)?;
        let source = join(&join(&item, FILES_DIR), rel.as_str());
        if !self.files.exists(&source) {
            return Err(Error::IncompleteItem);
        }
        let restored = unique_restore_path(&*self.files, &rel);
        move_path(&*self.files, &source, &restored)?;

        let snapshots = join(&join(&item, HISTORY_DIR), rel.as_str());
        if self.files.exists(&snapshots) {
            self.history.attach_snapshots(&snapshots, &restored)?;
        }

        let _record_removed = self.files.remove_file(&join(&item, REL_PATH_FILE));
        let _item_removed = remove_empty_tree(&*self.files, &item);
        Ok(restored)
    }

    pub fn delete(&self, id: &str) -> Result<(), Error> {
        let item = self.item_dir(id)?;
        self.files.remove_dir_all(&item)?;
        Ok(())
    }

    pub fn empty(&self) -> u32 {
        let Ok(entries) = self.files.list(&trash_root()) else {
            return 0;
        };
        let removed = entries
            .iter()
            .filter(|entry| {
                self.remove_item(&join(&trash_root(), &entry.name), entry.metadata.kind)
            })
            .count();
        u32::try_from(removed).unwrap_or(u32::MAX)
    }

    pub fn purge_older_than(&self, max_age: Duration) -> u32 {
        let Ok(entries) = self.files.list(&trash_root()) else {
            return 0;
        };
        let now = self.clock.now();
        let mut removed: u32 = 0;
        for entry in entries {
            let Some(deleted_at) = timestamp_millis(&entry.name) else {
                continue;
            };
            if now.elapsed_since(deleted_at) <= max_age {
                continue;
            }
            if self.remove_item(&join(&trash_root(), &entry.name), entry.metadata.kind) {
                removed += 1;
            }
        }
        removed
    }

    pub fn purge_expired(&self) -> u32 {
        self.purge_older_than(PURGE_AGE)
    }

    fn describe(&self, id: &str) -> Option<TrashItem> {
        let deleted_at = timestamp_millis(id)?;
        let item = join(&trash_root(), id);
        let rel = item_rel(&*self.files, &item).ok()?;
        let content = join(&join(&item, FILES_DIR), rel.as_str());
        Some(TrashItem {
            name: rel.file_name().to_owned(),
            is_dir: self.is_directory(&content),
            has_history: self
                .files
                .exists(&join(&join(&item, HISTORY_DIR), rel.as_str())),
            path: rel,
            deleted_at,
            id: id.to_owned(),
        })
    }

    fn item_dir(&self, id: &str) -> Result<RelativePath, Error> {
        let valid = !id.is_empty()
            && !id.contains(['/', '\\'])
            && !id.contains("..")
            && timestamp_millis(id).is_some_and(|stamp| stamp.as_millis() > 0);
        if !valid {
            return Err(Error::InvalidId);
        }
        let item = join(&trash_root(), id);
        if !self.is_directory(&item) {
            return Err(Error::MissingItem);
        }
        Ok(item)
    }

    fn is_directory(&self, path: &RelativePath) -> bool {
        self.files
            .metadata(path)
            .is_ok_and(|metadata| metadata.kind == EntryKind::Directory)
    }

    fn remove_item(&self, item: &RelativePath, kind: EntryKind) -> bool {
        let removed = if kind == EntryKind::Directory {
            self.files.remove_dir_all(item)
        } else {
            self.files.remove_file(item)
        };
        removed.is_ok()
    }

    fn new_item(&self) -> RelativePath {
        let millis = self.clock.now().as_millis();
        let root = trash_root();
        loop {
            let attempt = self.counter.fetch_add(1, Ordering::Relaxed);
            let candidate = join(&root, &format!("{millis}-{attempt}"));
            if !self.files.exists(&candidate) {
                return candidate;
            }
        }
    }
}

fn trash_root() -> RelativePath {
    RelativePath::new(TRASH_ROOT).expect("the trash root is a valid relative path")
}

fn move_path(files: &dyn FileSystem, from: &RelativePath, to: &RelativePath) -> Result<(), Error> {
    if let Some(parent) = to.parent() {
        files.create_dir_all(&parent)?;
    }
    files.rename(from, to)?;
    Ok(())
}

fn remove_empty_tree(files: &dyn FileSystem, directory: &RelativePath) -> bool {
    let Ok(entries) = files.list(directory) else {
        return false;
    };
    let mut empty = true;
    for entry in entries {
        if entry.metadata.kind != EntryKind::Directory
            || !remove_empty_tree(files, &join(directory, &entry.name))
        {
            empty = false;
        }
    }
    empty && files.remove_dir_all(directory).is_ok()
}
