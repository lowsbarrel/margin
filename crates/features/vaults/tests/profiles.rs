use std::sync::Arc;

use margin_kernel::{FileSystem, Secret};
use margin_test_support::MemoryFileSystem;
use margin_vaults::{Error, VaultProfileInfo, VaultService};

const FIXTURE_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const SECOND_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";
const FIXTURE_VAULT_ID: &str = "209947b258e347c7f68ea97be6a866a7771d2363696ff1fa585d6d28ae45e1af";

fn service() -> (Arc<MemoryFileSystem>, VaultService) {
    let memory = Arc::new(MemoryFileSystem::new());
    let file_system: Arc<dyn FileSystem> = Arc::<MemoryFileSystem>::clone(&memory);
    (memory, VaultService::new(file_system))
}

fn mnemonic() -> Secret<String> {
    Secret::new(String::from(FIXTURE_MNEMONIC))
}

fn info(name: &str, vault_path: &str) -> VaultProfileInfo {
    VaultProfileInfo {
        name: name.to_owned(),
        vault_path: vault_path.to_owned(),
    }
}

#[test]
fn creates_and_lists_profiles() {
    let (_memory, service) = service();
    service
        .save_profile("Notes", "C:/Users/test/Vault", &mnemonic())
        .expect("the profile saves");
    service
        .save_profile("Work", "D:/Work/Archive", &mnemonic())
        .expect("the profile saves");

    let listed = service.load_profiles().expect("the registry reads");
    assert_eq!(
        listed.profiles,
        vec![
            info("Notes", "C:/Users/test/Vault"),
            info("Work", "D:/Work/Archive")
        ]
    );
    assert_eq!(listed.last_used.as_deref(), Some("D:/Work/Archive"));
}

#[test]
fn updates_a_profile_saved_under_another_spelling_of_its_path() {
    let (_memory, service) = service();
    service
        .save_profile("Notes", "C:\\Users\\test\\Vault", &mnemonic())
        .expect("the profile saves");
    service
        .save_profile("Notes renamed", "C:/Users/test/Vault", &mnemonic())
        .expect("the profile saves");

    let listed = service.load_profiles().expect("the registry reads");
    assert_eq!(
        listed.profiles,
        vec![info("Notes renamed", "C:/Users/test/Vault")]
    );
}

#[test]
fn renames_a_profile() {
    let (_memory, service) = service();
    service
        .save_profile("Notes", "C:/Users/test/Vault", &mnemonic())
        .expect("the profile saves");
    service
        .rename_profile("C:\\Users\\test\\Vault", "Journal")
        .expect("the profile renames");

    let listed = service.load_profiles().expect("the registry reads");
    assert_eq!(
        listed.profiles,
        vec![info("Journal", "C:/Users/test/Vault")]
    );

    let error = service
        .rename_profile("D:/Work", "Journal")
        .expect_err("an unknown profile cannot be renamed");
    assert!(matches!(error, Error::ProfileNotFound(_)));
}

#[test]
fn deletes_a_profile_and_moves_the_last_used_marker() {
    let (_memory, service) = service();
    service
        .save_profile("Notes", "C:/Users/test/Vault", &mnemonic())
        .expect("the profile saves");
    service
        .save_profile("Work", "D:/Work/Archive", &mnemonic())
        .expect("the profile saves");

    service
        .delete_profile("D:/Work/Archive")
        .expect("the profile deletes");
    let listed = service.load_profiles().expect("the registry reads");
    assert_eq!(listed.profiles, vec![info("Notes", "C:/Users/test/Vault")]);
    assert_eq!(listed.last_used.as_deref(), Some("C:/Users/test/Vault"));

    service
        .delete_profile("D:/Work/Archive")
        .expect("deleting an unknown profile is a no-op");
    service
        .delete_profile("C:/Users/test/Vault")
        .expect("the profile deletes");
    let empty = service.load_profiles().expect("the registry reads");
    assert!(empty.profiles.is_empty());
    assert_eq!(empty.last_used, None);
}

#[test]
fn keeps_the_device_key_across_instances() {
    let (memory, service) = service();
    service
        .save_profile("Notes", "C:/Users/test/Vault", &mnemonic())
        .expect("the profile saves");
    drop(service);

    let file_system: Arc<dyn FileSystem> = Arc::<MemoryFileSystem>::clone(&memory);
    let reopened = VaultService::new(file_system);
    let listed = reopened.load_profiles().expect("the registry reads");
    assert_eq!(listed.profiles, vec![info("Notes", "C:/Users/test/Vault")]);
    assert_eq!(memory.contents("device.key").map(|key| key.len()), Some(32));
}

#[test]
fn names_a_new_session_after_its_folder() {
    let (_memory, service) = service();
    service
        .save_session("C:/Users/me/My Notes", &mnemonic())
        .expect("the session saves");

    let listed = service.load_profiles().expect("the registry reads");
    assert_eq!(
        listed.profiles,
        vec![info("My Notes", "C:/Users/me/My Notes")]
    );
    assert_eq!(listed.last_used.as_deref(), Some("C:/Users/me/My Notes"));
}

#[test]
fn keeps_the_name_of_an_existing_profile_when_a_session_saves_it() {
    let (_memory, service) = service();
    service
        .save_profile("Notes", "C:/Users/test/Vault", &mnemonic())
        .expect("the profile saves");
    service
        .save_session(
            "C:\\Users\\test\\Vault",
            &Secret::new(String::from(SECOND_MNEMONIC)),
        )
        .expect("the session saves");

    let listed = service.load_profiles().expect("the registry reads");
    assert_eq!(listed.profiles, vec![info("Notes", "C:/Users/test/Vault")]);
    let vault = service
        .unlock("C:/Users/test/Vault")
        .expect("the second mnemonic derives");
    assert_ne!(vault.id.as_str(), FIXTURE_VAULT_ID);
}

#[test]
fn unlocks_a_profile_and_reveals_its_mnemonic() {
    let (_memory, service) = service();
    service
        .save_profile("Notes", "C:/Users/test/Vault", &mnemonic())
        .expect("the profile saves");
    service
        .save_profile("Work", "D:/Work/Archive", &mnemonic())
        .expect("the profile saves");

    let locked = service
        .reveal_mnemonic("C:/Users/test/Vault")
        .expect_err("a locked vault reveals nothing");
    assert!(matches!(locked, Error::Locked));

    let vault = service
        .unlock("C:/Users/test/Vault")
        .expect("the profile unlocks");
    assert_eq!(vault.id.as_str(), FIXTURE_VAULT_ID);
    let revealed = service
        .reveal_mnemonic("C:/Users/test/Vault")
        .expect("the unlocked vault reveals");
    assert_eq!(revealed.expose(), FIXTURE_MNEMONIC);

    let other = service
        .reveal_mnemonic("D:/Work/Archive")
        .expect_err("another vault is locked");
    assert!(matches!(other, Error::Locked));
    assert_eq!(
        service
            .load_session()
            .expect("the session reads")
            .map(|session| session.name),
        Some("Notes".to_owned())
    );
}

#[test]
fn clearing_a_session_forgets_the_last_used_profile() {
    let (_memory, service) = service();
    service
        .save_profile("Notes", "C:/Users/test/Vault", &mnemonic())
        .expect("the profile saves");
    service
        .save_profile("Work", "D:/Work/Archive", &mnemonic())
        .expect("the profile saves");
    service
        .unlock("D:/Work/Archive")
        .expect("the profile unlocks");

    service.clear_session().expect("the session clears");
    let listed = service.load_profiles().expect("the registry reads");
    assert_eq!(listed.last_used, None);
    assert_eq!(
        service
            .load_session()
            .expect("the session reads")
            .map(|session| session.name),
        Some("Notes".to_owned())
    );
    let locked = service
        .reveal_mnemonic("D:/Work/Archive")
        .expect_err("a cleared session is locked");
    assert!(matches!(locked, Error::Locked));
}

#[test]
fn rejects_an_unknown_profile() {
    let (_memory, service) = service();
    let error = service
        .unlock("D:/Work/Archive")
        .expect_err("an unknown profile cannot unlock");
    assert!(matches!(error, Error::ProfileNotFound(_)));
}

#[test]
fn rejects_a_profile_whose_mnemonic_does_not_parse() {
    let (_memory, service) = service();
    service
        .save_profile(
            "Notes",
            "C:/Users/test/Vault",
            &Secret::new(String::from("not a mnemonic")),
        )
        .expect("the profile saves");
    let error = service
        .unlock("C:/Users/test/Vault")
        .expect_err("an invalid mnemonic cannot unlock");
    assert!(matches!(error, Error::Crypto(_)));
}
