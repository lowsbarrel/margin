use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("{message}")]
    Parse { message: String },
    #[error("{message}")]
    TooLarge { message: String },
    #[error("`{value}` is not a #rrggbb colour")]
    InvalidColor { value: String },
}
