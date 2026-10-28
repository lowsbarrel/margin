use std::fmt;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use margin_crypto::{VaultKey, decrypt, encrypt};
use margin_kernel::{FileSystem, RelativePath};
use serde::de::DeserializeOwned;

use crate::domain::{self, AppSettings, WorkspaceState};
use crate::error::Error;

const SETTINGS_PATH: &str = ".margin/settings.enc";
const WORKSPACE_PATH: &str = ".margin/workspace.enc";

#[derive(Clone)]
pub struct Settings {
    files: Arc<dyn FileSystem>,
}

impl Settings {
    #[must_use]
    pub fn new(files: Arc<dyn FileSystem>) -> Self {
        Self { files }
    }

    pub fn save(&self, key: &VaultKey, settings: &AppSettings) -> Result<(), Error> {
        let json = serde_json::to_vec(settings)?;
        self.files
            .write(&RelativePath::new(SETTINGS_PATH)?, &encrypt(key, &json))?;
        Ok(())
    }

    pub fn load(&self, key: &VaultKey) -> Result<Option<AppSettings>, Error> {
        self.read_encrypted(key, SETTINGS_PATH)
    }

    pub fn export_string(key: &VaultKey, settings: &AppSettings) -> Result<String, Error> {
        let json = serde_json::to_vec(settings)?;
        Ok(B64.encode(encrypt(key, &json)))
    }

    pub fn import_string(key: &VaultKey, encoded: &str) -> Result<AppSettings, Error> {
        let encrypted = B64.decode(encoded.trim())?;
        let plaintext = decrypt(key, &encrypted)?;
        let settings: AppSettings = serde_json::from_slice(&plaintext)?;
        domain::validate(&settings)?;
        Ok(settings)
    }

    pub fn save_workspace(&self, key: &VaultKey, state: &WorkspaceState) -> Result<(), Error> {
        let json = serde_json::to_vec(state)?;
        self.files
            .write(&RelativePath::new(WORKSPACE_PATH)?, &encrypt(key, &json))?;
        Ok(())
    }

    pub fn load_workspace(&self, key: &VaultKey) -> Result<Option<WorkspaceState>, Error> {
        self.read_encrypted(key, WORKSPACE_PATH)
    }

    fn read_encrypted<T: DeserializeOwned>(
        &self,
        key: &VaultKey,
        path: &str,
    ) -> Result<Option<T>, Error> {
        let path = RelativePath::new(path)?;
        if !self.files.exists(&path) {
            return Ok(None);
        }
        let plaintext = decrypt(key, &self.files.read(&path)?)?;
        Ok(Some(serde_json::from_slice(&plaintext)?))
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings").finish_non_exhaustive()
    }
}
