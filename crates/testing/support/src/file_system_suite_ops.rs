use margin_kernel::{EntryKind, RelativePath, StoreError, UnixMillis};

use crate::suite::{Factory, failed, names, ok, path};

pub(crate) fn rename_moves_files_and_directories(factory: Factory<'_>) {
    let fs = factory();
    ok(fs.write(&path("from.md"), b"body"), "writing a file");
    ok(
        fs.rename(&path("from.md"), &path("nested/deep/to.md")),
        "renaming into a new parent",
    );
    assert!(!fs.exists(&path("from.md")));
    assert_eq!(
        ok(
            fs.read(&path("nested/deep/to.md")),
            "reading the moved file"
        ),
        b"body"
    );
    ok(fs.write(&path("old/one.md"), b"one"), "writing a file");
    ok(
        fs.write(&path("old/inner/two.md"), b"two"),
        "writing a nested file",
    );
    ok(
        fs.rename(&path("old"), &path("new")),
        "renaming a directory",
    );
    assert!(!fs.exists(&path("old")));
    assert_eq!(
        ok(fs.read(&path("new/one.md")), "reading a moved file"),
        b"one"
    );
    assert_eq!(
        ok(
            fs.read(&path("new/inner/two.md")),
            "reading a deeply moved file"
        ),
        b"two"
    );
    let missing = failed(
        fs.rename(&path("absent.md"), &path("other.md")),
        "renaming a missing file",
    );
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
    ok(
        fs.rename(&path("new"), &path("new")),
        "renaming a directory onto itself",
    );
}

pub(crate) fn rename_refuses_conflicts_and_itself(factory: Factory<'_>) {
    let fs = factory();
    ok(fs.write(&path("from.md"), b"source"), "writing a file");
    ok(fs.write(&path("to.md"), b"target"), "writing a file");
    let conflict = failed(
        fs.rename(&path("from.md"), &path("to.md")),
        "renaming onto a file",
    );
    assert!(
        matches!(conflict, StoreError::AlreadyExists(_)),
        "unexpected error: {conflict}"
    );
    assert_eq!(
        ok(fs.read(&path("from.md")), "reading the source"),
        b"source"
    );
    assert_eq!(ok(fs.read(&path("to.md")), "reading the target"), b"target");
    ok(
        fs.create_dir_all(&path("folder/inner")),
        "creating nested directories",
    );
    let itself = failed(
        fs.rename(&path("folder"), &path("folder/inner")),
        "moving a directory into itself",
    );
    assert!(
        matches!(itself, StoreError::InvalidData(_)),
        "unexpected error: {itself}"
    );
    assert!(fs.exists(&path("folder/inner")));
    ok(fs.write(&path("note.md"), b"case"), "writing a file");
    ok(
        fs.rename(&path("note.md"), &path("Note.md")),
        "renaming the case of a file",
    );
    let listed = names(&ok(fs.list(&RelativePath::root()), "listing the root"));
    assert!(
        listed.iter().any(|name| name == "Note.md"),
        "the new spelling is listed: {listed:?}"
    );
    assert_eq!(
        ok(fs.read(&path("Note.md")), "reading the renamed file"),
        b"case"
    );
}

pub(crate) fn copy_file_duplicates_a_file(factory: Factory<'_>) {
    let fs = factory();
    ok(fs.write(&path("source.md"), b"copy me"), "writing a file");
    ok(
        fs.copy_file(&path("source.md"), &path("nested/target.md")),
        "copying into a new parent",
    );
    assert_eq!(
        ok(fs.read(&path("source.md")), "reading the source"),
        b"copy me"
    );
    assert_eq!(
        ok(fs.read(&path("nested/target.md")), "reading the copy"),
        b"copy me"
    );
    let conflict = failed(
        fs.copy_file(&path("source.md"), &path("nested/target.md")),
        "copying onto a file",
    );
    assert!(
        matches!(conflict, StoreError::AlreadyExists(_)),
        "unexpected error: {conflict}"
    );
    let missing = failed(
        fs.copy_file(&path("absent.md"), &path("copy.md")),
        "copying a missing file",
    );
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
    let directory = failed(
        fs.copy_file(&path("nested"), &path("other.md")),
        "copying a directory",
    );
    assert!(
        matches!(directory, StoreError::InvalidData(_)),
        "unexpected error: {directory}"
    );
}

pub(crate) fn removal_checks_the_entry_kind(factory: Factory<'_>) {
    let fs = factory();
    ok(fs.write(&path("gone.md"), b"x"), "writing a file");
    ok(fs.remove_file(&path("gone.md")), "removing a file");
    assert!(!fs.exists(&path("gone.md")));
    let missing = failed(fs.remove_file(&path("gone.md")), "removing a missing file");
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
    ok(
        fs.create_dir_all(&path("tree/sub")),
        "creating a directory tree",
    );
    ok(
        fs.write(&path("tree/sub/inside.md"), b"x"),
        "writing a nested file",
    );
    let file = failed(
        fs.remove_dir_all(&path("tree/sub/inside.md")),
        "removing a file as a directory",
    );
    assert!(
        matches!(file, StoreError::InvalidData(_)),
        "unexpected error: {file}"
    );
    assert!(fs.exists(&path("tree/sub/inside.md")));
    let directory = failed(
        fs.remove_file(&path("tree")),
        "removing a directory as a file",
    );
    assert!(
        matches!(directory, StoreError::InvalidData(_)),
        "unexpected error: {directory}"
    );
    assert!(fs.exists(&path("tree")));
    ok(fs.remove_dir_all(&path("tree")), "removing a directory");
    assert!(!fs.exists(&path("tree")));
    assert!(!fs.exists(&path("tree/sub/inside.md")));
    let absent = failed(
        fs.remove_dir_all(&path("tree")),
        "removing a missing directory",
    );
    assert!(
        matches!(absent, StoreError::NotFound(_)),
        "unexpected error: {absent}"
    );
}

pub(crate) fn set_modified_round_trips(factory: Factory<'_>) {
    let fs = factory();
    let stamp = UnixMillis::from_millis(1_700_000_000_123);
    ok(fs.write(&path("stamped.md"), b"x"), "writing a file");
    ok(
        fs.set_modified(&path("stamped.md"), stamp),
        "stamping a file",
    );
    assert_eq!(
        ok(fs.metadata(&path("stamped.md")), "reading file metadata").modified,
        stamp
    );
    ok(fs.create_dir_all(&path("stamped")), "creating a directory");
    ok(
        fs.set_modified(&path("stamped"), stamp),
        "stamping a directory",
    );
    assert_eq!(
        ok(fs.metadata(&path("stamped")), "reading directory metadata").modified,
        stamp
    );
    let missing = failed(
        fs.set_modified(&path("absent.md"), stamp),
        "stamping a missing file",
    );
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
}

pub(crate) fn root_is_never_written_or_removed(factory: Factory<'_>) {
    let fs = factory();
    let root = RelativePath::root();
    assert_eq!(
        ok(fs.metadata(&root), "reading root metadata").kind,
        EntryKind::Directory
    );
    assert!(fs.exists(&root));
    let empty = names(&ok(fs.list(&root), "listing the root"));
    assert!(empty.is_empty(), "a fresh root is empty: {empty:?}");
    ok(fs.create_dir_all(&root), "creating the root");
    let written = failed(fs.write(&root, b"x"), "writing the root");
    assert!(
        matches!(written, StoreError::InvalidData(_)),
        "unexpected error: {written}"
    );
    let read = failed(fs.read(&root), "reading the root");
    assert!(
        matches!(read, StoreError::InvalidData(_)),
        "unexpected error: {read}"
    );
    let file = failed(fs.remove_file(&root), "removing the root as a file");
    assert!(
        matches!(file, StoreError::InvalidData(_)),
        "unexpected error: {file}"
    );
    let removed = failed(fs.remove_dir_all(&root), "removing the root");
    assert!(
        matches!(removed, StoreError::InvalidData(_)),
        "unexpected error: {removed}"
    );
    assert_eq!(ok(fs.list(&root), "listing the root again").len(), 0);
}
