use margin_kernel::StoreError;
use thiserror::Error;

use crate::domain::SessionId;

#[derive(Debug, Error)]
pub enum Error {
    #[error("open a vault before starting a terminal")]
    NoVault,
    #[error("terminal {0} is not running")]
    NotRunning(SessionId),
    #[error(transparent)]
    Host(#[from] StoreError),
}
