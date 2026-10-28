use std::sync::Arc;

use margin_kernel::{EntryKind, FileSystem, RelativePath, StoreError};

use crate::file_system_suite_ops::{
    copy_file_duplicates_a_file, removal_checks_the_entry_kind, rename_moves_files_and_directories,
    rename_refuses_conflicts_and_itself, root_is_never_written_or_removed,
    set_modified_round_trips,
};
use crate::suite::{Factory, failed, kind_of, names, ok, path, size_of};

pub fn check_file_system(factory: impl Fn() -> Arc<dyn FileSystem>) {
    let factory: Factory<'_> = &factory;
    write_creates_parents_and_replaces(factory);
    read_distinguishes_missing_from_directories(factory);
    metadata_reports_kind_and_size(factory);
    list_reports_direct_children(factory);
    create_dir_all_is_idempotent(factory);
    write_private_writes_and_replaces(factory);
    rename_moves_files_and_directories(factory);
    rename_refuses_conflicts_and_itself(factory);
    copy_file_duplicates_a_file(factory);
    removal_checks_the_entry_kind(factory);
    set_modified_round_trips(factory);
    root_is_never_written_or_removed(factory);
}

fn write_creates_parents_and_replaces(factory: Factory<'_>) {
    let fs = factory();
    let target = path("notes/sub/plan.md");
    ok(
        fs.write(&target, b"first"),
        "writing through missing parents",
    );
    assert_eq!(ok(fs.read(&target), "reading the written file"), b"first");
    ok(fs.write(&target, b"second"), "overwriting a file");
    assert_eq!(
        ok(fs.read(&target), "reading the overwritten file"),
        b"second"
    );
    ok(fs.write(&path("plan.md"), b"top"), "writing a sibling");
    assert_eq!(
        ok(fs.read(&target), "reading the untouched file"),
        b"second"
    );
    ok(fs.write(&path("file.md"), b"x"), "writing a file");
    let error = failed(
        fs.write(&path("file.md/child.md"), b"y"),
        "writing below a file",
    );
    assert!(
        matches!(error, StoreError::AlreadyExists(_)),
        "unexpected error: {error}"
    );
}

fn read_distinguishes_missing_from_directories(factory: Factory<'_>) {
    let fs = factory();
    let missing = failed(fs.read(&path("absent.md")), "reading a missing file");
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
    ok(fs.create_dir_all(&path("folder")), "creating a directory");
    let directory = failed(fs.read(&path("folder")), "reading a directory");
    assert!(
        matches!(directory, StoreError::InvalidData(_)),
        "unexpected error: {directory}"
    );
}

fn metadata_reports_kind_and_size(factory: Factory<'_>) {
    let fs = factory();
    ok(fs.write(&path("notes/plan.md"), b"12345"), "writing a file");
    let file = ok(fs.metadata(&path("notes/plan.md")), "reading file metadata");
    assert_eq!(file.kind, EntryKind::File);
    assert_eq!(file.size, 5);
    let directory = ok(fs.metadata(&path("notes")), "reading directory metadata");
    assert_eq!(directory.kind, EntryKind::Directory);
    let missing = failed(fs.metadata(&path("absent")), "reading missing metadata");
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
}

fn list_reports_direct_children(factory: Factory<'_>) {
    let fs = factory();
    ok(fs.write(&path("keep.md"), b"abc"), "writing a file");
    ok(
        fs.write(&path("nested/deep.md"), b"d"),
        "writing a nested file",
    );
    ok(fs.write(&path(".hidden.md"), b"h"), "writing a hidden file");
    ok(
        fs.create_dir_all(&path("empty")),
        "creating an empty directory",
    );
    let entries = ok(fs.list(&RelativePath::root()), "listing the root");
    assert_eq!(
        names(&entries),
        [".hidden.md", "empty", "keep.md", "nested"]
    );
    assert_eq!(kind_of(&entries, "keep.md"), EntryKind::File);
    assert_eq!(kind_of(&entries, ".hidden.md"), EntryKind::File);
    assert_eq!(kind_of(&entries, "nested"), EntryKind::Directory);
    assert_eq!(kind_of(&entries, "empty"), EntryKind::Directory);
    assert_eq!(size_of(&entries, "keep.md"), 3);
    assert_eq!(
        names(&ok(fs.list(&path("nested")), "listing a child")),
        ["deep.md"]
    );
    let missing = failed(fs.list(&path("absent")), "listing a missing directory");
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
    let file = failed(fs.list(&path("keep.md")), "listing a file");
    assert!(
        matches!(file, StoreError::InvalidData(_)),
        "unexpected error: {file}"
    );
}

fn create_dir_all_is_idempotent(factory: Factory<'_>) {
    let fs = factory();
    ok(
        fs.create_dir_all(&path("a/b")),
        "creating nested directories",
    );
    assert_eq!(
        ok(fs.metadata(&path("a/b")), "reading directory metadata").kind,
        EntryKind::Directory
    );
    ok(fs.create_dir_all(&path("a/b")), "repeating create_dir_all");
    ok(
        fs.create_dir_all(&RelativePath::root()),
        "creating the root",
    );
    ok(fs.write(&path("file.md"), b"x"), "writing a file");
    let conflict = failed(
        fs.create_dir_all(&path("file.md")),
        "creating a directory on a file",
    );
    assert!(
        matches!(conflict, StoreError::AlreadyExists(_)),
        "unexpected error: {conflict}"
    );
    let nested = failed(
        fs.create_dir_all(&path("file.md/child")),
        "creating below a file",
    );
    assert!(
        matches!(nested, StoreError::AlreadyExists(_)),
        "unexpected error: {nested}"
    );
}

fn write_private_writes_and_replaces(factory: Factory<'_>) {
    let fs = factory();
    let key = path(".margin/device.key");
    ok(
        fs.write_private(&key, b"first key"),
        "writing a private file",
    );
    assert_eq!(ok(fs.read(&key), "reading the private file"), b"first key");
    let written = ok(fs.metadata(&key), "reading private metadata");
    assert_eq!(written.kind, EntryKind::File);
    assert_eq!(written.size, 9);
    ok(
        fs.write_private(&key, b"second key"),
        "replacing a private file",
    );
    assert_eq!(
        ok(fs.read(&key), "reading the replaced private file"),
        b"second key"
    );
}
