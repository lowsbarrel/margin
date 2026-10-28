mod domain;
mod error;
mod service;

pub use domain::{
    LegacySession, VaultProfile, VaultProfileInfo, VaultProfiles, VaultProfilesInfo,
    normalise_slashes, profile_name_from_vault_path,
};
pub use error::Error;
pub use service::VaultService;
