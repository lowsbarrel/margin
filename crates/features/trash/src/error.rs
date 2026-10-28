use margin_kernel::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("invalid trash id")]
    InvalidId,
    #[error("trash item no longer exists")]
    MissingItem,
    #[error("trash item holds nothing")]
    EmptyItem,
    #[error("trash item has no recorded path")]
    UnrecordedPath,
    #[error("trash item has an invalid recorded path")]
    InvalidRecordedPath,
    #[error("trash item is missing its contents")]
    IncompleteItem,
    #[error("refusing to delete the vault root")]
    VaultRoot,
}
