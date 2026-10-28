use std::fs;
use std::path::Path;

use margin_kernel::StoreError;

use crate::atomic_write::{Privacy, atomic_write};
use crate::io_error::from_io;

pub fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| from_io(&error, path))?;
    }
    atomic_write(path, contents, Privacy::OwnerOnly)
}
