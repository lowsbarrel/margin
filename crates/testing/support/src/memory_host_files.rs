use std::collections::BTreeMap;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use margin_kernel::{DirEntry, EntryKind, HostFiles, HostWriter, Metadata, StoreError, UnixMillis};
use parking_lot::Mutex;

#[derive(Clone, Debug, Default)]
pub struct MemoryHostFiles {
    nodes: Arc<Mutex<BTreeMap<PathBuf, Node>>>,
}

#[derive(Clone, Debug)]
enum Node {
    File(Vec<u8>),
    Directory,
}

impl Node {
    fn metadata(&self) -> Metadata {
        let (kind, size) = match self {
            Self::File(contents) => (
                EntryKind::File,
                u64::try_from(contents.len()).unwrap_or(u64::MAX),
            ),
            Self::Directory => (EntryKind::Directory, 0),
        };
        Metadata {
            kind,
            size,
            modified: UnixMillis::default(),
        }
    }
}

impl MemoryHostFiles {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, path: &Path, contents: &[u8]) {
        self.write(path, contents).expect("fixture writes succeed");
    }

    #[must_use]
    pub fn contents(&self, path: &Path) -> Option<Vec<u8>> {
        match self.nodes.lock().get(path) {
            Some(Node::File(contents)) => Some(contents.clone()),
            _ => None,
        }
    }

    #[must_use]
    pub fn paths(&self) -> Vec<PathBuf> {
        self.nodes.lock().keys().cloned().collect()
    }
}

fn create_parents(nodes: &mut BTreeMap<PathBuf, Node>, path: &Path) -> Result<(), StoreError> {
    for ancestor in path
        .ancestors()
        .skip(1)
        .filter(|ancestor| ancestor.parent().is_some())
    {
        if let Some(Node::File(_)) = nodes.get(ancestor) {
            return Err(StoreError::AlreadyExists(ancestor.display().to_string()));
        }
        if !nodes.contains_key(ancestor) {
            nodes.insert(ancestor.to_path_buf(), Node::Directory);
        }
    }
    Ok(())
}

impl HostFiles for MemoryHostFiles {
    fn read(&self, path: &Path) -> Result<Vec<u8>, StoreError> {
        match self.nodes.lock().get(path) {
            Some(Node::File(contents)) => Ok(contents.clone()),
            Some(Node::Directory) => Err(StoreError::InvalidData(format!(
                "{} is a directory",
                path.display()
            ))),
            None => Err(StoreError::NotFound(path.display().to_string())),
        }
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), StoreError> {
        let mut nodes = self.nodes.lock();
        if matches!(nodes.get(path), Some(Node::Directory)) {
            return Err(StoreError::AlreadyExists(path.display().to_string()));
        }
        create_parents(&mut nodes, path)?;
        nodes.insert(path.to_path_buf(), Node::File(contents.to_vec()));
        Ok(())
    }

    fn create(&self, path: &Path) -> Result<Box<dyn HostWriter>, StoreError> {
        self.write(path, &[])?;
        Ok(Box::new(MemoryWriter {
            nodes: Arc::clone(&self.nodes),
            path: path.to_path_buf(),
            position: 0,
        }))
    }

    fn metadata(&self, path: &Path) -> Result<Metadata, StoreError> {
        match self.nodes.lock().get(path) {
            Some(node) => Ok(node.metadata()),
            None => Err(StoreError::NotFound(path.display().to_string())),
        }
    }

    fn list(&self, directory: &Path) -> Result<Vec<DirEntry>, StoreError> {
        let nodes = self.nodes.lock();
        match nodes.get(directory) {
            Some(Node::Directory) => {}
            Some(Node::File(_)) => {
                return Err(StoreError::InvalidData(format!(
                    "{} is a file",
                    directory.display()
                )));
            }
            None => return Err(StoreError::NotFound(directory.display().to_string())),
        }
        let entries = nodes
            .iter()
            .filter(|(path, _)| path.parent() == Some(directory))
            .map(|(path, node)| DirEntry {
                name: path
                    .file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
                metadata: node.metadata(),
            })
            .collect();
        Ok(entries)
    }
}

struct MemoryWriter {
    nodes: Arc<Mutex<BTreeMap<PathBuf, Node>>>,
    path: PathBuf,
    position: u64,
}

impl MemoryWriter {
    fn length(&self) -> u64 {
        match self.nodes.lock().get(&self.path) {
            Some(Node::File(contents)) => u64::try_from(contents.len()).unwrap_or(u64::MAX),
            _ => 0,
        }
    }
}

impl Write for MemoryWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let start = usize::try_from(self.position).map_err(io::Error::other)?;
        let mut nodes = self.nodes.lock();
        let Some(Node::File(contents)) = nodes.get_mut(&self.path) else {
            return Err(io::Error::other(format!(
                "{} is not open",
                self.path.display()
            )));
        };
        let end = start.saturating_add(buf.len());
        if contents.len() < end {
            contents.resize(end, 0);
        }
        contents[start..end].copy_from_slice(buf);
        self.position = self
            .position
            .saturating_add(u64::try_from(buf.len()).unwrap_or(u64::MAX));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for MemoryWriter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let length = i64::try_from(self.length()).unwrap_or(i64::MAX);
        let current = i64::try_from(self.position).unwrap_or(i64::MAX);
        let target = match pos {
            SeekFrom::Start(offset) => i64::try_from(offset).unwrap_or(i64::MAX),
            SeekFrom::End(offset) => length.saturating_add(offset),
            SeekFrom::Current(offset) => current.saturating_add(offset),
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                String::from("cannot seek before the start"),
            ));
        }
        self.position = u64::try_from(target).unwrap_or(0);
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> PathBuf {
        PathBuf::from(raw)
    }

    #[test]
    fn writes_read_back_through_a_writer() {
        let files = MemoryHostFiles::new();
        let target = path("/vault/notes.md");
        {
            let mut writer = files.create(&target).expect("the file is created");
            writer.write_all(b"hello").expect("the writer writes");
            writer.seek(SeekFrom::Start(0)).expect("the writer seeks");
            writer.write_all(b"HELLO").expect("the writer overwrites");
        }
        assert_eq!(files.contents(&target), Some(b"HELLO".to_vec()));
        assert_eq!(
            files
                .metadata(&path("/vault"))
                .expect("an implicit parent")
                .kind,
            EntryKind::Directory
        );
        assert_eq!(
            files.list(&path("/vault")).expect("the parent lists").len(),
            1
        );
    }

    #[test]
    fn inserts_fixtures_with_their_parents() {
        let files = MemoryHostFiles::new();
        files.insert(&path("/vault/a/b.md"), b"b");
        assert_eq!(files.contents(&path("/vault/a/b.md")), Some(b"b".to_vec()));
        assert_eq!(files.paths().len(), 3);
    }
}
