use std::io;

use margin_kernel::StoreError;
use margin_trash::Error as TrashError;
use thiserror::Error;
use zip::result::ZipError;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Trash(#[from] TrashError),
    #[error(transparent)]
    Zip(#[from] ZipError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("\"{name}\" already exists")]
    Occupied { name: String },
    #[error("destination is inside the source directory")]
    NestedDestination,
    #[error("directory tree is too deep to copy")]
    TooDeep,
    #[error("failed to resolve the copy destination")]
    UnresolvedDestination,
    #[error("not a directory: {0}")]
    NotADirectory(String),
    #[error("{path} is not valid UTF-8 text")]
    NotText { path: String },
}
