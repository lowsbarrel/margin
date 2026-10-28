use std::io::{Seek, Write};
use std::path::Path;

use crate::{DirEntry, Metadata, StoreError};

pub trait HostWriter: Write + Seek + Send {}

impl<W: Write + Seek + Send> HostWriter for W {}

pub trait HostFiles: Send + Sync {
    fn read(&self, path: &Path) -> Result<Vec<u8>, StoreError>;

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), StoreError>;

    fn create(&self, path: &Path) -> Result<Box<dyn HostWriter>, StoreError>;

    fn metadata(&self, path: &Path) -> Result<Metadata, StoreError>;

    fn list(&self, directory: &Path) -> Result<Vec<DirEntry>, StoreError>;
}
