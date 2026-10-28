use margin_kernel::StoreError;
use margin_trash::Error as TrashError;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Trash(#[from] TrashError),
    #[error("invalid attachments folder: {0}")]
    InvalidFolder(String),
    #[error("could not find a free name for the attachment")]
    NoFreeName,
}
