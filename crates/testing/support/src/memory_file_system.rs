use std::collections::BTreeMap;
use std::sync::Arc;

use margin_kernel::{
    Clock, DirEntry, EntryKind, FileSystem, Metadata, RelativePath, StoreError, UnixMillis,
};
use parking_lot::Mutex;

use crate::FixedClock;

#[derive(Clone, Debug)]
enum Node {
    File {
        contents: Vec<u8>,
        modified: UnixMillis,
    },
    Directory {
        modified: UnixMillis,
    },
}

impl Node {
    fn metadata(&self) -> Metadata {
        match self {
            Self::File { contents, modified } => Metadata {
                kind: EntryKind::File,
                size: u64::try_from(contents.len()).unwrap_or(u64::MAX),
                modified: *modified,
            },
            Self::Directory { modified } => Metadata {
                kind: EntryKind::Directory,
                size: 0,
                modified: *modified,
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct MemoryFileSystem {
    nodes: Mutex<BTreeMap<RelativePath, Node>>,
    clock: Option<Arc<FixedClock>>,
}

impl MemoryFileSystem {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_clock(clock: Arc<FixedClock>) -> Self {
        Self {
            nodes: Mutex::default(),
            clock: Some(clock),
        }
    }

    pub fn insert(&self, path: &str, contents: &[u8]) {
        let path = RelativePath::new(path).expect("fixture paths are valid");
        self.write(&path, contents).expect("fixture writes succeed");
    }

    #[must_use]
    pub fn contents(&self, path: &str) -> Option<Vec<u8>> {
        let path = RelativePath::new(path).ok()?;
        match self.nodes.lock().get(&path) {
            Some(Node::File { contents, .. }) => Some(contents.clone()),
            _ => None,
        }
    }

    #[must_use]
    pub fn paths(&self) -> Vec<String> {
        self.nodes.lock().keys().map(ToString::to_string).collect()
    }

    fn now(&self) -> UnixMillis {
        self.clock
            .as_ref()
            .map_or_else(UnixMillis::default, |clock| clock.now())
    }

    fn ensure_parents(
        nodes: &mut BTreeMap<RelativePath, Node>,
        path: &RelativePath,
        now: UnixMillis,
    ) -> Result<(), StoreError> {
        let mut ancestors = Vec::new();
        let mut current = path.parent();
        while let Some(parent) = current {
            if parent.is_root() {
                break;
            }
            current = parent.parent();
            ancestors.push(parent);
        }
        for ancestor in ancestors.into_iter().rev() {
            match nodes.get(&ancestor) {
                Some(Node::Directory { .. }) => {}
                Some(Node::File { .. }) => {
                    return Err(StoreError::AlreadyExists(ancestor.to_string()));
                }
                None => {
                    nodes.insert(ancestor, Node::Directory { modified: now });
                }
            }
        }
        Ok(())
    }

    fn descendants(nodes: &BTreeMap<RelativePath, Node>, path: &RelativePath) -> Vec<RelativePath> {
        nodes
            .keys()
            .filter(|key| key.starts_with(path) && *key != path)
            .cloned()
            .collect()
    }
}

impl FileSystem for MemoryFileSystem {
    fn read(&self, path: &RelativePath) -> Result<Vec<u8>, StoreError> {
        if path.is_root() {
            return Err(StoreError::InvalidData(String::from(
                "the root is a directory",
            )));
        }
        match self.nodes.lock().get(path) {
            Some(Node::File { contents, .. }) => Ok(contents.clone()),
            Some(Node::Directory { .. }) => {
                Err(StoreError::InvalidData(format!("{path} is a directory")))
            }
            None => Err(StoreError::NotFound(path.to_string())),
        }
    }

    fn write(&self, path: &RelativePath, contents: &[u8]) -> Result<(), StoreError> {
        if path.is_root() {
            return Err(StoreError::InvalidData(String::from(
                "cannot write the root",
            )));
        }
        let now = self.now();
        let mut nodes = self.nodes.lock();
        if matches!(nodes.get(path), Some(Node::Directory { .. })) {
            return Err(StoreError::AlreadyExists(path.to_string()));
        }
        Self::ensure_parents(&mut nodes, path, now)?;
        nodes.insert(
            path.clone(),
            Node::File {
                contents: contents.to_vec(),
                modified: now,
            },
        );
        Ok(())
    }

    fn metadata(&self, path: &RelativePath) -> Result<Metadata, StoreError> {
        if path.is_root() {
            return Ok(Metadata {
                kind: EntryKind::Directory,
                size: 0,
                modified: UnixMillis::default(),
            });
        }
        self.nodes
            .lock()
            .get(path)
            .map(Node::metadata)
            .ok_or_else(|| StoreError::NotFound(path.to_string()))
    }

    fn list(&self, directory: &RelativePath) -> Result<Vec<DirEntry>, StoreError> {
        let nodes = self.nodes.lock();
        match nodes.get(directory) {
            Some(Node::Directory { .. }) => {}
            Some(Node::File { .. }) => {
                return Err(StoreError::InvalidData(format!("{directory} is a file")));
            }
            None if directory.is_root() => {}
            None => return Err(StoreError::NotFound(directory.to_string())),
        }
        let entries = nodes
            .iter()
            .filter(|(key, _)| key.parent().as_ref() == Some(directory))
            .map(|(key, node)| DirEntry {
                name: key.file_name().to_owned(),
                metadata: node.metadata(),
            })
            .collect();
        Ok(entries)
    }

    fn create_dir_all(&self, path: &RelativePath) -> Result<(), StoreError> {
        if path.is_root() {
            return Ok(());
        }
        let now = self.now();
        let mut nodes = self.nodes.lock();
        Self::ensure_parents(&mut nodes, path, now)?;
        match nodes.get(path) {
            Some(Node::Directory { .. }) => Ok(()),
            Some(Node::File { .. }) => Err(StoreError::AlreadyExists(path.to_string())),
            None => {
                nodes.insert(path.clone(), Node::Directory { modified: now });
                Ok(())
            }
        }
    }

    fn rename(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError> {
        let now = self.now();
        let mut nodes = self.nodes.lock();
        if !nodes.contains_key(from) {
            return Err(StoreError::NotFound(from.to_string()));
        }
        if from == to {
            return Ok(());
        }
        let case_only = from.as_str().eq_ignore_ascii_case(to.as_str());
        if to.starts_with(from) {
            return Err(StoreError::InvalidData(format!(
                "cannot move {from} into itself"
            )));
        }
        if nodes.contains_key(to) && !case_only {
            return Err(StoreError::AlreadyExists(to.to_string()));
        }
        Self::ensure_parents(&mut nodes, to, now)?;
        let mut moved = Self::descendants(&nodes, from);
        moved.push(from.clone());
        for old in moved {
            let Some(node) = nodes.remove(&old) else {
                continue;
            };
            let suffix = old.strip_prefix(from).unwrap_or_default();
            let new = to
                .join(suffix.as_str())
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            nodes.insert(new, node);
        }
        Ok(())
    }

    fn copy_file(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError> {
        if self.metadata(to).is_ok() {
            return Err(StoreError::AlreadyExists(to.to_string()));
        }
        let contents = self.read(from)?;
        self.write(to, &contents)
    }

    fn remove_file(&self, path: &RelativePath) -> Result<(), StoreError> {
        if path.is_root() {
            return Err(StoreError::InvalidData(String::from(
                "the root is a directory",
            )));
        }
        let mut nodes = self.nodes.lock();
        match nodes.get(path) {
            Some(Node::File { .. }) => {
                nodes.remove(path);
                Ok(())
            }
            Some(Node::Directory { .. }) => {
                Err(StoreError::InvalidData(format!("{path} is a directory")))
            }
            None => Err(StoreError::NotFound(path.to_string())),
        }
    }

    fn remove_dir_all(&self, path: &RelativePath) -> Result<(), StoreError> {
        if path.is_root() {
            return Err(StoreError::InvalidData(String::from(
                "the root cannot be removed",
            )));
        }
        let mut nodes = self.nodes.lock();
        match nodes.get(path) {
            Some(Node::Directory { .. }) => {}
            Some(Node::File { .. }) => {
                return Err(StoreError::InvalidData(format!("{path} is a file")));
            }
            None => return Err(StoreError::NotFound(path.to_string())),
        }
        for key in Self::descendants(&nodes, path) {
            nodes.remove(&key);
        }
        nodes.remove(path);
        Ok(())
    }

    fn set_modified(&self, path: &RelativePath, modified: UnixMillis) -> Result<(), StoreError> {
        match self.nodes.lock().get_mut(path) {
            Some(
                Node::File {
                    modified: stamp, ..
                }
                | Node::Directory { modified: stamp },
            ) => {
                *stamp = modified;
                Ok(())
            }
            None => Err(StoreError::NotFound(path.to_string())),
        }
    }
}
