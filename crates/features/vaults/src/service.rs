use std::fmt;
use std::sync::Arc;

use margin_crypto::{DerivedVault, VaultKey, decrypt, derive_vault, encrypt, random_key};
use margin_kernel::{FileSystem, RelativePath, Secret, StoreError};
use parking_lot::Mutex;

use crate::domain::{
    LegacySession, VaultProfile, VaultProfileInfo, VaultProfiles, VaultProfilesInfo,
    normalise_slashes, profile_name_from_vault_path,
};
use crate::error::Error;

const DEVICE_KEY_FILE: &str = "device.key";
const LEGACY_SESSION_FILE: &str = "session.enc";
const PROFILES_FILE: &str = "profiles.enc";
const QUARANTINED_PROFILES_FILE: &str = "profiles.enc.bad";
const DEVICE_KEY_BYTES: usize = 32;

fn registry_path(name: &str) -> RelativePath {
    RelativePath::new(name).expect("the registry file names are valid relative paths")
}

pub struct VaultService {
    app_data: Arc<dyn FileSystem>,
    unlocked_vault_path: Mutex<Option<String>>,
}

impl VaultService {
    #[must_use]
    pub fn new(app_data: Arc<dyn FileSystem>) -> Self {
        Self {
            app_data,
            unlocked_vault_path: Mutex::new(None),
        }
    }

    pub fn load_profiles(&self) -> Result<VaultProfilesInfo, Error> {
        let _guard = self.unlocked_vault_path.lock();
        self.read_registry().map(VaultProfiles::into_info)
    }

    pub fn save_profile(
        &self,
        name: &str,
        vault_path: &str,
        mnemonic: &Secret<String>,
    ) -> Result<(), Error> {
        let _guard = self.unlocked_vault_path.lock();
        self.upsert_profile(name, vault_path, mnemonic)
    }

    pub fn rename_profile(&self, vault_path: &str, name: &str) -> Result<(), Error> {
        let _guard = self.unlocked_vault_path.lock();
        let mut registry = self.read_registry()?;
        let path = normalise_slashes(vault_path);
        let Some(profile) = registry
            .profiles
            .iter_mut()
            .find(|profile| profile.vault_path == path)
        else {
            return Err(Error::ProfileNotFound(path));
        };
        name.clone_into(&mut profile.name);
        self.write_registry(&registry)
    }

    pub fn delete_profile(&self, vault_path: &str) -> Result<(), Error> {
        let _guard = self.unlocked_vault_path.lock();
        let mut registry = self.read_registry()?;
        let path = normalise_slashes(vault_path);
        registry
            .profiles
            .retain(|profile| profile.vault_path != path);
        if registry.last_used.as_deref() == Some(path.as_str()) {
            registry.last_used = registry
                .profiles
                .first()
                .map(|profile| profile.vault_path.clone());
        }
        self.write_registry(&registry)
    }

    pub fn save_session(&self, vault_path: &str, mnemonic: &Secret<String>) -> Result<(), Error> {
        let _guard = self.unlocked_vault_path.lock();
        let path = normalise_slashes(vault_path);
        let registry = self.read_registry()?;
        let name = registry
            .profiles
            .iter()
            .find(|profile| profile.vault_path == path)
            .map_or_else(
                || profile_name_from_vault_path(vault_path),
                |profile| profile.name.clone(),
            );
        self.upsert_profile(&name, &path, mnemonic)
    }

    pub fn load_session(&self) -> Result<Option<VaultProfileInfo>, Error> {
        let _guard = self.unlocked_vault_path.lock();
        Ok(self.read_registry()?.session())
    }

    pub fn clear_session(&self) -> Result<(), Error> {
        let mut unlocked_vault_path = self.unlocked_vault_path.lock();
        *unlocked_vault_path = None;
        let mut registry = self.read_registry()?;
        registry.last_used = None;
        self.write_registry(&registry)
    }

    pub fn unlock(&self, vault_path: &str) -> Result<DerivedVault, Error> {
        let mut unlocked_vault_path = self.unlocked_vault_path.lock();
        let mut registry = self.read_registry()?;
        let path = normalise_slashes(vault_path);
        let mnemonic = registry
            .profiles
            .iter()
            .find(|profile| profile.vault_path == path)
            .map(|profile| profile.mnemonic.clone())
            .ok_or_else(|| Error::ProfileNotFound(path.clone()))?;
        let vault = derive_vault(mnemonic.expose())?;
        registry.last_used = Some(path.clone());
        self.write_registry(&registry)?;
        *unlocked_vault_path = Some(path);
        Ok(vault)
    }

    pub fn reveal_mnemonic(&self, vault_path: &str) -> Result<Secret<String>, Error> {
        let unlocked_vault_path = self.unlocked_vault_path.lock();
        let path = normalise_slashes(vault_path);
        if unlocked_vault_path.as_deref() != Some(path.as_str()) {
            return Err(Error::Locked);
        }
        self.read_registry()?
            .profiles
            .iter()
            .find(|profile| profile.vault_path == path)
            .map(|profile| profile.mnemonic.clone())
            .ok_or(Error::ProfileNotFound(path))
    }

    fn upsert_profile(
        &self,
        name: &str,
        vault_path: &str,
        mnemonic: &Secret<String>,
    ) -> Result<(), Error> {
        let mut registry = self.read_registry()?;
        let path = normalise_slashes(vault_path);
        match registry
            .profiles
            .iter_mut()
            .find(|profile| profile.vault_path == path)
        {
            Some(profile) => {
                name.clone_into(&mut profile.name);
                profile.mnemonic = mnemonic.clone();
                profile.vault_path.clone_from(&path);
            }
            None => registry.profiles.push(VaultProfile {
                name: name.to_owned(),
                mnemonic: mnemonic.clone(),
                vault_path: path.clone(),
            }),
        }
        registry.last_used = Some(path);
        self.write_registry(&registry)
    }

    fn read_registry(&self) -> Result<VaultProfiles, Error> {
        let key = self.device_key()?;
        let path = registry_path(PROFILES_FILE);
        let encrypted = match self.app_data.read(&path) {
            Ok(encrypted) => encrypted,
            Err(StoreError::NotFound(_)) => return self.migrate_legacy_session(&key),
            Err(error) => return Err(error.into()),
        };
        let Ok(plaintext) = decrypt(&key, &encrypted) else {
            return Ok(self.quarantine_profiles());
        };
        match serde_json::from_slice::<VaultProfiles>(&plaintext) {
            Ok(registry) => Ok(registry.normalised()),
            Err(_) => Ok(self.quarantine_profiles()),
        }
    }

    fn write_registry(&self, registry: &VaultProfiles) -> Result<(), Error> {
        let key = self.device_key()?;
        let json = serde_json::to_vec(registry)
            .map_err(|error| Error::RegistryEncoding(error.to_string()))?;
        let encrypted = encrypt(&key, &json);
        let path = registry_path(PROFILES_FILE);
        self.app_data.write(&path, &encrypted)?;
        Ok(())
    }

    fn device_key(&self) -> Result<VaultKey, Error> {
        let path = registry_path(DEVICE_KEY_FILE);
        let existing = match self.app_data.read(&path) {
            Ok(bytes) => <[u8; DEVICE_KEY_BYTES]>::try_from(bytes.as_slice())
                .ok()
                .map(VaultKey::from_bytes),
            Err(StoreError::NotFound(_)) => None,
            Err(error) => return Err(error.into()),
        };
        if let Some(key) = existing {
            return Ok(key);
        }
        let key = random_key();
        self.app_data.write_private(&path, key.as_bytes())?;
        Ok(key)
    }

    fn quarantine_profiles(&self) -> VaultProfiles {
        let path = registry_path(PROFILES_FILE);
        let quarantined = registry_path(QUARANTINED_PROFILES_FILE);
        self.app_data
            .rename(&path, &quarantined)
            .unwrap_or_default();
        VaultProfiles::default()
    }

    fn migrate_legacy_session(&self, key: &VaultKey) -> Result<VaultProfiles, Error> {
        let path = registry_path(LEGACY_SESSION_FILE);
        let encrypted = match self.app_data.read(&path) {
            Ok(encrypted) => encrypted,
            Err(StoreError::NotFound(_)) => return Ok(VaultProfiles::default()),
            Err(error) => return Err(error.into()),
        };
        let Ok(plaintext) = decrypt(key, &encrypted) else {
            return Ok(VaultProfiles::default());
        };
        let Ok(legacy) = serde_json::from_slice::<LegacySession>(&plaintext) else {
            return Ok(VaultProfiles::default());
        };

        let vault_path = normalise_slashes(&legacy.vault_path);
        let registry = VaultProfiles {
            profiles: vec![VaultProfile {
                name: profile_name_from_vault_path(&legacy.vault_path),
                mnemonic: legacy.mnemonic,
                vault_path: vault_path.clone(),
            }],
            last_used: Some(vault_path),
        };
        self.write_registry(&registry)?;
        self.app_data.remove_file(&path).unwrap_or_default();
        Ok(registry)
    }
}

impl fmt::Debug for VaultService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VaultService").finish_non_exhaustive()
    }
}
