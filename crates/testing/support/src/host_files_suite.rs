use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use margin_kernel::{EntryKind, HostFiles, StoreError};

use crate::suite::{failed, kind_of, names, ok, size_of};

struct Suite<'suite> {
    files: &'suite dyn Fn() -> Arc<dyn HostFiles>,
    directory: &'suite Path,
}

impl Suite<'_> {
    fn host(&self) -> Arc<dyn HostFiles> {
        (self.files)()
    }

    fn root(&self, case: &str) -> PathBuf {
        self.directory.join(case)
    }
}

pub fn check_host_files(files: impl Fn() -> Arc<dyn HostFiles>, directory: &Path) {
    let suite = Suite {
        files: &files,
        directory,
    };
    write_read_and_overwrite(&suite);
    read_reports_missing_and_directories(&suite);
    metadata_reports_kind_and_size(&suite);
    create_truncates_through_a_writer(&suite);
    list_reports_direct_children(&suite);
}

fn write_read_and_overwrite(suite: &Suite<'_>) {
    let host = suite.host();
    let root = suite.root("write");
    let target = root.join("notes/sub/plan.md");
    ok(
        host.write(&target, b"first"),
        "writing through missing parents",
    );
    assert_eq!(ok(host.read(&target), "reading the written file"), b"first");
    ok(host.write(&target, b"second"), "overwriting a file");
    assert_eq!(
        ok(host.read(&target), "reading the overwritten file"),
        b"second"
    );
    let parent = ok(
        host.metadata(&root.join("notes")),
        "reading an implicit parent",
    );
    assert_eq!(parent.kind, EntryKind::Directory);
}

fn read_reports_missing_and_directories(suite: &Suite<'_>) {
    let host = suite.host();
    let root = suite.root("read");
    let missing = failed(host.read(&root.join("absent.md")), "reading a missing file");
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
    ok(
        host.write(&root.join("folder/hold.md"), b"x"),
        "writing a nested file",
    );
    let directory = failed(host.read(&root.join("folder")), "reading a directory");
    assert!(
        matches!(directory, StoreError::InvalidData(_)),
        "unexpected error: {directory}"
    );
}

fn metadata_reports_kind_and_size(suite: &Suite<'_>) {
    let host = suite.host();
    let root = suite.root("metadata");
    let target = root.join("notes/plan.md");
    ok(host.write(&target, b"12345"), "writing a file");
    let file = ok(host.metadata(&target), "reading file metadata");
    assert_eq!(file.kind, EntryKind::File);
    assert_eq!(file.size, 5);
    let directory = ok(
        host.metadata(&root.join("notes")),
        "reading directory metadata",
    );
    assert_eq!(directory.kind, EntryKind::Directory);
    let missing = failed(
        host.metadata(&root.join("absent")),
        "reading missing metadata",
    );
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
}

fn create_truncates_through_a_writer(suite: &Suite<'_>) {
    let host = suite.host();
    let root = suite.root("create");
    let target = root.join("device.key");
    ok(
        host.write(&target, b"stale contents"),
        "writing a stale file",
    );
    {
        let mut writer = ok(host.create(&target), "creating the file");
        writer.write_all(b"secret").expect("the writer writes");
        writer.seek(SeekFrom::Start(0)).expect("the writer seeks");
        writer.write_all(b"SECRET").expect("the writer overwrites");
    }
    assert_eq!(
        ok(host.read(&target), "reading the created file"),
        b"SECRET"
    );
    let nested = root.join("nested/deep/config.json");
    {
        let mut writer = ok(host.create(&nested), "creating through missing parents");
        writer.write_all(b"{}").expect("the writer writes");
    }
    assert_eq!(ok(host.read(&nested), "reading the nested file"), b"{}");
}

fn list_reports_direct_children(suite: &Suite<'_>) {
    let host = suite.host();
    let root = suite.root("listing");
    ok(host.write(&root.join("keep.md"), b"abc"), "writing a file");
    ok(
        host.write(&root.join("nested/deep.md"), b"d"),
        "writing a nested file",
    );
    ok(
        host.write(&root.join(".hidden.md"), b"h"),
        "writing a hidden file",
    );
    let entries = ok(host.list(&root), "listing the directory");
    assert_eq!(names(&entries), [".hidden.md", "keep.md", "nested"]);
    assert_eq!(kind_of(&entries, "keep.md"), EntryKind::File);
    assert_eq!(kind_of(&entries, "nested"), EntryKind::Directory);
    assert_eq!(size_of(&entries, "keep.md"), 3);
    assert_eq!(
        names(&ok(host.list(&root.join("nested")), "listing a child")),
        ["deep.md"]
    );
    let missing = failed(
        host.list(&root.join("absent")),
        "listing a missing directory",
    );
    assert!(
        matches!(missing, StoreError::NotFound(_)),
        "unexpected error: {missing}"
    );
    let file = failed(host.list(&root.join("keep.md")), "listing a file");
    assert!(
        matches!(file, StoreError::InvalidData(_)),
        "unexpected error: {file}"
    );
}
