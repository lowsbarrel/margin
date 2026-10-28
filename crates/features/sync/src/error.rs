use margin_crypto::CryptoError;
use margin_kernel::{PathError, StoreError};
use margin_retry::Retryable;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("invalid manifest: {0}")]
    Manifest(String),
}

impl Retryable for Error {
    fn is_transient(&self) -> bool {
        matches!(self, Self::Store(StoreError::Unavailable(_)))
    }
}
