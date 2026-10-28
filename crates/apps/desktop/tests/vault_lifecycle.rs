use std::fs;
use std::path::Path;

use margin_crypto::derive_vault;
use margin_desktop::{ContainerError, app_services, open_vault};
use margin_kernel::{RelativePath, Secret};
use margin_settings::AppSettings;
use tempfile::TempDir;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const OTHER_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";

fn temp_dir() -> TempDir {
    TempDir::new().expect("a temporary directory is created")
}

fn folder(root: &Path) -> &str {
    root.to_str().expect("the temporary path is UTF-8")
}

fn seed(root: &Path) {
    fs::write(root.join("alpha.md"), "# Alpha\n").expect("the first note is written");
    fs::create_dir_all(root.join("notes")).expect("the folder is created");
    fs::write(root.join("notes/beta.md"), "# Beta\n").expect("the nested note is written");
}

#[test]
fn a_saved_profile_unlocks_and_opens_the_seeded_vault_after_a_relaunch() {
    let (data, vault_root) = (temp_dir(), temp_dir());
    seed(vault_root.path());
    let root = folder(vault_root.path());
    app_services(data.path().to_path_buf())
        .vaults
        .save_profile("Notes", root, &Secret::new(MNEMONIC.to_owned()))
        .expect("the profile is saved");

    let app = app_services(data.path().to_path_buf());
    let profiles = app.vaults.load_profiles().expect("the registry reads");
    assert_eq!(profiles.profiles.len(), 1);
    assert_eq!(profiles.profiles[0].name, "Notes");
    assert_eq!(profiles.last_used.as_deref(), Some(root));
    let vault = app.vaults.unlock(root).expect("the saved profile unlocks");
    assert_eq!(
        vault,
        derive_vault(MNEMONIC).expect("the mnemonic is valid")
    );

    let opened =
        open_vault(&app, "Notes".to_owned(), vault_root.path(), &vault).expect("the vault opens");
    let mut listed: Vec<String> = opened
        .services
        .files
        .walk_directory(&RelativePath::root(), false)
        .into_iter()
        .map(|entry| entry.path.as_str().to_owned())
        .collect();
    listed.sort();
    assert_eq!(listed, ["alpha.md", "notes", "notes/beta.md"]);
    assert_eq!(
        opened
            .services
            .files
            .read_file(&RelativePath::new("notes/beta.md").expect("the path is valid"))
            .expect("the note reads"),
        b"# Beta\n"
    );
}

#[test]
fn another_mnemonic_cannot_open_a_vault_with_saved_settings() {
    let (data, vault_root) = (temp_dir(), temp_dir());
    seed(vault_root.path());
    let app = app_services(data.path().to_path_buf());
    let vault = derive_vault(MNEMONIC).expect("the mnemonic is valid");
    let opened =
        open_vault(&app, "Notes".to_owned(), vault_root.path(), &vault).expect("the vault opens");
    let settings = AppSettings {
        theme: Some("dark".to_owned()),
        ..AppSettings::default()
    };
    opened
        .services
        .settings
        .save(&opened.services.key, &settings)
        .expect("the settings are saved");
    drop(opened);

    let other = derive_vault(OTHER_MNEMONIC).expect("the other mnemonic is valid");
    let error = open_vault(&app, "Notes".to_owned(), vault_root.path(), &other)
        .expect_err("the wrong key cannot read the vault settings");
    assert!(matches!(error, ContainerError::Settings(_)), "{error:?}");

    let reopened = open_vault(&app, "Notes".to_owned(), vault_root.path(), &vault)
        .expect("the right key still opens the vault");
    assert_eq!(
        reopened.services.app_settings.read().theme.as_deref(),
        Some("dark")
    );
}
