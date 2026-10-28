use margin_kernel::StoreError;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    #[error("the model must not be empty")]
    EmptyModel,
    #[error("the base URL must be http or https")]
    UnsupportedScheme,
    #[error("AI is not configured")]
    NotConfigured,
    #[error("a question is required")]
    EmptyQuestion,
    #[error("the endpoint returned no models")]
    NoModels,
    #[error("Missing required argument: {0}")]
    MissingArgument(String),
    #[error("Invalid regular expression: {0}")]
    InvalidPattern(String),
    #[error("{0} escapes the vault")]
    OutsideVault(String),
    #[error("{path} has only {lines} lines")]
    NoteTooShort { path: String, lines: usize },
    #[error("Unknown tool: {0}")]
    UnknownTool(String),
    #[error("The model kept calling tools without answering. Try a narrower question.")]
    IterationsExhausted,
    #[error("cancelled")]
    Cancelled,
    #[error("{message}")]
    Http {
        status: Option<u16>,
        message: String,
    },
    #[error("{0}")]
    Failed(String),
    #[error("{0}")]
    Store(#[from] StoreError),
}

impl Error {
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        match self {
            Self::Http { status, .. } => *status,
            _ => None,
        }
    }
}
