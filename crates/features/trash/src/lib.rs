mod domain;
mod error;
mod service;

pub use domain::{PURGE_AGE, TrashItem};
pub use error::Error;
pub use service::TrashService;
