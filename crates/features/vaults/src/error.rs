use margin_crypto::CryptoError;
use margin_kernel::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] StoreError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error("the profile registry could not be encoded: {0}")]
    RegistryEncoding(String),
    #[error("no vault profile for `{0}`")]
    ProfileNotFound(String),
    #[error("the vault is locked")]
    Locked,
}
