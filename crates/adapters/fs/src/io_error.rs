use std::io::{Error, ErrorKind};
use std::path::Path;

use margin_kernel::StoreError;

pub(crate) fn from_io(error: &Error, path: &Path) -> StoreError {
    let display = path.display().to_string();
    match error.kind() {
        ErrorKind::NotFound => StoreError::NotFound(display),
        ErrorKind::AlreadyExists | ErrorKind::NotADirectory => StoreError::AlreadyExists(display),
        ErrorKind::PermissionDenied => StoreError::PermissionDenied(display),
        _ => StoreError::Other(format!("{display}: {error}")),
    }
}
