use margin_kernel::{PathError, StoreError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("history storage failed: {0}")]
    Store(#[from] StoreError),
    #[error("invalid path: {0}")]
    Path(#[from] PathError),
    #[error("invalid snapshot filename: {0}")]
    InvalidSnapshotFilename(String),
    #[error("no free snapshot name for {0}")]
    NameExhausted(String),
}

impl Error {
    pub(crate) fn into_store_error(self) -> StoreError {
        match self {
            Self::Store(error) => error,
            Self::Path(error) => StoreError::InvalidData(error.to_string()),
            other => StoreError::Other(other.to_string()),
        }
    }
}
