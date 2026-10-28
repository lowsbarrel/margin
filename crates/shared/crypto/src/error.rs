use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum CryptoError {
    #[error("`{0}` is not a valid BIP-39 mnemonic")]
    InvalidMnemonic(String),
    #[error("the blob is shorter than a nonce")]
    TruncatedBlob,
    #[error("the blob does not authenticate under this key")]
    NotAuthentic,
}
