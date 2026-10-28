use crate::{RelativePath, StoreError, UnixMillis};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub kind: EntryKind,
    pub size: u64,
    pub modified: UnixMillis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub metadata: Metadata,
}

pub trait FileSystem: Send + Sync {
    fn read(&self, path: &RelativePath) -> Result<Vec<u8>, StoreError>;

    fn write(&self, path: &RelativePath, contents: &[u8]) -> Result<(), StoreError>;

    fn write_private(&self, path: &RelativePath, contents: &[u8]) -> Result<(), StoreError> {
        self.write(path, contents)
    }

    fn metadata(&self, path: &RelativePath) -> Result<Metadata, StoreError>;

    fn list(&self, directory: &RelativePath) -> Result<Vec<DirEntry>, StoreError>;

    fn create_dir_all(&self, path: &RelativePath) -> Result<(), StoreError>;

    fn rename(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError>;

    fn copy_file(&self, from: &RelativePath, to: &RelativePath) -> Result<(), StoreError>;

    fn remove_file(&self, path: &RelativePath) -> Result<(), StoreError>;

    fn remove_dir_all(&self, path: &RelativePath) -> Result<(), StoreError>;

    fn set_modified(&self, path: &RelativePath, modified: UnixMillis) -> Result<(), StoreError>;

    fn exists(&self, path: &RelativePath) -> bool {
        self.metadata(path).is_ok()
    }
}
