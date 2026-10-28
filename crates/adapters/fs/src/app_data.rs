use std::path::PathBuf;

use margin_kernel::StoreError;

const APPLICATION_IDENTIFIER: &str = "com.lowsbarrel.margin";

pub fn app_data_dir() -> Result<PathBuf, StoreError> {
    match dirs::data_dir() {
        Some(directory) => Ok(directory.join(APPLICATION_IDENTIFIER)),
        None => Err(StoreError::Unavailable(String::from(
            "the platform has no data directory",
        ))),
    }
}
