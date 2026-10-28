use margin_ask::Error as LlmError;
use margin_crypto::CryptoError;
use margin_kernel::{PathError, StoreError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("settings storage failed: {0}")]
    Store(#[from] StoreError),
    #[error("invalid path: {0}")]
    Path(#[from] PathError),
    #[error("encryption failed: {0}")]
    Crypto(#[from] CryptoError),
    #[error("serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid base64: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("invalid llm configuration: {0}")]
    Llm(#[from] LlmError),
    #[error("invalid settings: {0}")]
    Invalid(String),
}
