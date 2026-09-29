use super::*;
use crate::fs::{VaultPathState, ensure_in_vault, ensure_in_vault_keep_name, path_to_string};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "margin-fs-test-{}-{tag}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn rename_refuses_an_existing_destination() {
    let dir = temp_dir("rename-conflict");
    let from = dir.join("from.md");
    let to = dir.join("to.md");
    fs::write(&from, "source").unwrap();
    fs::write(&to, "target").unwrap();

    let err = rename_entry_at(&from, &to).unwrap_err();
    assert!(err.contains("to.md"), "error must name the target: {err}");
    assert_eq!(fs::read_to_string(&to).unwrap(), "target");
    assert_eq!(fs::read_to_string(&from).unwrap(), "source");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_into_a_free_path_still_renames() {
    let dir = temp_dir("rename-free");
    let from = dir.join("from.md");
    let to = dir.join("nested").join("to.md");
    fs::write(&from, "source").unwrap();

    rename_entry_at(&from, &to).unwrap();

    assert!(!from.exists());
    assert_eq!(fs::read_to_string(&to).unwrap(), "source");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn case_only_rename_of_the_same_file_changes_its_spelling() {
    let dir = temp_dir("rename-case");
    let lower = dir.join("note.md");
    fs::write(&lower, "body").unwrap();
    // A case-sensitive filesystem has no `Note.md`, so there is no rename to exercise.
    if !dir.join("Note.md").exists() {
        fs::remove_dir_all(&dir).ok();
        return;
    }
    let vault = VaultPathState(Mutex::new(path_to_string(dir.clone())));
    let from = ensure_in_vault(&path_to_string(lower), &vault).unwrap();
    let to = ensure_in_vault_keep_name(&path_to_string(dir.join("Note.md")), &vault).unwrap();

    rename_entry_at(&from, &to).unwrap();

    let names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["Note.md"]);
    assert_eq!(fs::read_to_string(dir.join("Note.md")).unwrap(), "body");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn case_varied_destination_is_a_conflict_when_both_spellings_exist() {
    let dir = temp_dir("rename-case-conflict");
    let lower = dir.join("note.md");
    let upper = dir.join("Note.md");
    fs::write(&lower, "lower").unwrap();
    fs::write(&upper, "upper").unwrap();
    // A case-insensitive filesystem just wrote the same file twice.
    if fs::read_to_string(&lower).unwrap() == "upper" {
        fs::remove_dir_all(&dir).ok();
        return;
    }

    assert!(rename_entry_at(&lower, &upper).is_err());
    assert_eq!(fs::read_to_string(&upper).unwrap(), "upper");
    assert_eq!(fs::read_to_string(&lower).unwrap(), "lower");

    fs::remove_dir_all(&dir).ok();
}

#[cfg(unix)]
#[test]
fn a_dangling_symlink_still_blocks_a_rename() {
    let dir = temp_dir("rename-symlink");
    let from = dir.join("from.md");
    fs::write(&from, "source").unwrap();
    let to = dir.join("to.md");
    std::os::unix::fs::symlink(dir.join("missing.md"), &to).unwrap();

    assert!(rename_entry_at(&from, &to).is_err());
    assert_eq!(fs::read_to_string(&from).unwrap(), "source");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn copy_and_import_refuse_an_existing_destination() {
    let dir = temp_dir("copy-conflict");
    let src = dir.join("inside.md");
    fs::write(&src, "source").unwrap();
    let outside = dir.join("outside.png");
    fs::write(&outside, "imported").unwrap();

    let existing = dir.join("target.md");
    fs::write(&existing, "target").unwrap();
    let err = copy_file_at(&src, &existing).unwrap_err();
    assert!(
        err.contains("target.md"),
        "error must name the target: {err}"
    );
    assert_eq!(fs::read_to_string(&existing).unwrap(), "target");

    let imported = dir.join("existing.png");
    fs::write(&imported, "keep").unwrap();
    let err = copy_file_at(&outside, &imported).unwrap_err();
    assert!(
        err.contains("existing.png"),
        "error must name the target: {err}"
    );
    assert_eq!(fs::read_to_string(&imported).unwrap(), "keep");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn copy_into_a_free_path_still_copies() {
    let dir = temp_dir("copy-free");
    let src = dir.join("inside.md");
    fs::write(&src, "source").unwrap();
    let dst = dir.join("nested").join("copy.md");

    copy_file_at(&src, &dst).unwrap();

    assert_eq!(fs::read_to_string(&dst).unwrap(), "source");
    assert_eq!(fs::read_to_string(&src).unwrap(), "source");

    fs::remove_dir_all(&dir).ok();
}
