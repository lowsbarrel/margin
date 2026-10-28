use std::fs;
use std::path::PathBuf;

use margin_kernel::StoreError;

use crate::io_error::from_io;

const APPLICATION_IDENTIFIER: &str = "com.lowsbarrel.margin";

pub fn app_data_dir() -> Result<PathBuf, StoreError> {
    let Some(directory) = dirs::data_dir() else {
        return Err(StoreError::Unavailable(String::from(
            "the platform has no data directory",
        )));
    };
    let directory = directory.join(APPLICATION_IDENTIFIER);
    fs::create_dir_all(&directory).map_err(|error| from_io(&error, &directory))?;
    Ok(directory)
}
