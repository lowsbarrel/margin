use std::path::Path;

use gpui_kit::{App, SharedString};
use margin_crypto::{DerivedVault, VaultId};
use margin_i18n::{t, t_with};
use margin_kernel::{HostFiles, Secret, StoreError};
use margin_vaults::VaultService;
use thiserror::Error;

use crate::passphrase::{self, PassphraseError};

const VAULT_ID_FILE: [&str; 2] = [".margin", "vault.id"];

#[derive(Debug, Error)]
pub(crate) enum OpenError {
    #[error(transparent)]
    Passphrase(#[from] PassphraseError),
    #[error("the passphrase does not match this vault")]
    WrongPassphrase,
    #[error(transparent)]
    Vaults(#[from] margin_vaults::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl OpenError {
    pub(crate) fn message(&self, cx: &App) -> SharedString {
        match self {
            Self::Passphrase(error) => t(cx, error.message_key()),
            Self::WrongPassphrase => t(cx, "login_error_wrong_passphrase"),
            Self::Vaults(_) | Self::Store(_) => {
                let detail = self.to_string();
                t_with(cx, "login_error_open_failed", &[("error", detail.as_str())])
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct NewVault {
    name: String,
    folder: String,
    passphrase: Secret<String>,
}

impl NewVault {
    pub(crate) const fn new(name: String, folder: String, passphrase: Secret<String>) -> Self {
        Self {
            name,
            folder,
            passphrase,
        }
    }
}

pub(crate) fn create(
    vaults: &VaultService,
    host: &dyn HostFiles,
    request: &NewVault,
) -> Result<DerivedVault, OpenError> {
    let (mnemonic, vault) = passphrase::derive(request.passphrase.expose())?;
    claim_folder(host, Path::new(&request.folder), &vault.id)?;
    vaults.save_profile(&request.name, &request.folder, &mnemonic)?;
    Ok(vaults.unlock(&request.folder)?)
}

pub(crate) fn open_saved(
    vaults: &VaultService,
    host: &dyn HostFiles,
    folder: &str,
) -> Result<DerivedVault, OpenError> {
    let vault = vaults.unlock(folder)?;
    claim_folder(host, Path::new(folder), &vault.id)?;
    Ok(vault)
}

fn claim_folder(host: &dyn HostFiles, root: &Path, id: &VaultId) -> Result<(), OpenError> {
    let path = VAULT_ID_FILE
        .iter()
        .fold(root.to_path_buf(), |path, part| path.join(part));
    match host.read(&path) {
        Ok(stored) if String::from_utf8_lossy(&stored).trim() == id.as_str() => Ok(()),
        Ok(_) => Err(OpenError::WrongPassphrase),
        Err(StoreError::NotFound(_)) => Ok(host.write(&path, id.as_str().as_bytes())?),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use margin_crypto::{derive_vault, generate_mnemonic};
    use margin_test_support::{MemoryFileSystem, MemoryHostFiles};

    use super::*;

    const WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    fn request(passphrase: &str) -> NewVault {
        NewVault {
            name: String::from("Notes"),
            folder: String::from("/home/me/Notes"),
            passphrase: Secret::new(passphrase.to_owned()),
        }
    }

    #[test]
    fn creating_a_vault_marks_the_folder_and_saves_the_profile() {
        let vaults = VaultService::new(Arc::new(MemoryFileSystem::new()));
        let host = MemoryHostFiles::new();
        let vault =
            create(&vaults, &host, &request(WORDS)).expect("a fresh folder accepts the vault");

        let expected = derive_vault(WORDS).expect("the fixture is valid");
        assert_eq!(vault, expected);
        assert_eq!(
            host.contents(Path::new("/home/me/Notes/.margin/vault.id")),
            Some(expected.id.as_str().as_bytes().to_vec())
        );
        let profiles = vaults.load_profiles().expect("the registry reads back");
        assert_eq!(profiles.profiles.len(), 1);
        assert_eq!(profiles.profiles[0].name, "Notes");
        assert_eq!(profiles.last_used.as_deref(), Some("/home/me/Notes"));
        assert_eq!(
            vaults
                .reveal_mnemonic("/home/me/Notes")
                .expect("the new vault is unlocked")
                .expose(),
            WORDS
        );
        let reopened =
            open_saved(&vaults, &host, "/home/me/Notes").expect("the saved vault reopens");
        assert_eq!(reopened, expected);
    }

    #[test]
    fn another_passphrase_cannot_take_over_a_marked_folder() {
        let vaults = VaultService::new(Arc::new(MemoryFileSystem::new()));
        let host = MemoryHostFiles::new();
        create(&vaults, &host, &request(WORDS)).expect("the first vault claims the folder");

        let other = generate_mnemonic();
        let error = create(&vaults, &host, &request(other.expose()))
            .expect_err("a different passphrase is rejected");
        assert!(matches!(error, OpenError::WrongPassphrase));
        assert_eq!(
            vaults
                .reveal_mnemonic("/home/me/Notes")
                .expect("the original profile is still unlocked")
                .expose(),
            WORDS
        );
    }

    #[test]
    fn rejects_an_invalid_passphrase_before_touching_disk() {
        let vaults = VaultService::new(Arc::new(MemoryFileSystem::new()));
        let host = MemoryHostFiles::new();
        let error = create(&vaults, &host, &request("not a passphrase"))
            .expect_err("invalid words are rejected");
        assert!(matches!(
            error,
            OpenError::Passphrase(PassphraseError::Invalid)
        ));
        assert!(host.paths().is_empty());
        assert!(
            vaults
                .load_profiles()
                .expect("the registry reads")
                .profiles
                .is_empty()
        );
    }
}
