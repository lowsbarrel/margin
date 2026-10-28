use std::fs;

use margin_crypto::derive_vault;
use margin_desktop::{app_services, open_vault};
use margin_kernel::{PageRequest, RelativePath};
use tempfile::TempDir;

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[test]
fn a_deleted_note_waits_in_the_trash_and_comes_back_with_its_history() {
    let data = TempDir::new().expect("the data directory is created");
    let root = TempDir::new().expect("the vault directory is created");
    fs::create_dir_all(root.path().join("notes")).expect("the folder is created");
    fs::write(root.path().join("notes/plan.md"), "draft").expect("the note is seeded");
    let app = app_services(data.path().to_path_buf());
    let vault = derive_vault(MNEMONIC).expect("the mnemonic is valid");
    let services = open_vault(&app, "Notes".to_owned(), root.path(), &vault)
        .expect("the vault opens")
        .services;
    let note = RelativePath::new("notes/plan.md").expect("the path is valid");
    services
        .history
        .save(&note, b"draft")
        .expect("the snapshot is saved");
    services
        .files
        .write_file(&note, b"final")
        .expect("the note is saved");

    services
        .files
        .delete_entry(&note)
        .expect("the note moves to the trash");
    assert!(!root.path().join("notes/plan.md").exists());
    assert!(
        services
            .history
            .list(&note)
            .expect("the history reads")
            .is_empty()
    );
    let trashed = services.trash.list(&PageRequest::default()).items;
    let [item] = trashed.as_slice() else {
        panic!("exactly one item is in the trash: {trashed:?}");
    };
    assert_eq!(item.path, note);
    assert!(item.has_history);

    let restored = services
        .trash
        .restore(&item.id)
        .expect("the trashed note restores");
    assert_eq!(restored, note);
    assert_eq!(
        fs::read_to_string(root.path().join("notes/plan.md")).expect("the note is back"),
        "final"
    );
    let snapshots = services.history.list(&note).expect("the history reads");
    let [snapshot] = snapshots.as_slice() else {
        panic!("the snapshot came back with the note: {snapshots:?}");
    };
    assert_eq!(
        services
            .history
            .read(&note, &snapshot.filename)
            .expect("the snapshot reads"),
        b"draft"
    );
    assert!(
        services
            .trash
            .list(&PageRequest::default())
            .items
            .is_empty()
    );
}
