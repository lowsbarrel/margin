use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use margin_files::{FilesService, SortBy};
use margin_kernel::{FileSystem, HistoryStore, RelativePath, UnixMillis};
use margin_test_support::{FixedClock, MemoryFileSystem, MemoryHistoryStore, MemoryHostFiles};
use margin_trash::TrashService;
use zip::ZipArchive;

const NOW: u64 = 1_700_000_000_123;

struct Fixture {
    vault: Arc<MemoryFileSystem>,
    host: Arc<MemoryHostFiles>,
    clock: Arc<FixedClock>,
    files: FilesService,
}

impl Fixture {
    fn new() -> Self {
        let clock = Arc::new(FixedClock::new(UnixMillis::from_millis(NOW)));
        let vault = Arc::new(MemoryFileSystem::with_clock(Arc::clone(&clock)));
        let host = Arc::new(MemoryHostFiles::new());
        let history: Arc<dyn HistoryStore> = Arc::new(MemoryHistoryStore::new(Arc::clone(&vault)));
        let trash = Arc::new(TrashService::new(
            Arc::<MemoryFileSystem>::clone(&vault),
            Arc::clone(&history),
            Arc::<FixedClock>::clone(&clock),
        ));
        let files = FilesService::new(
            Arc::<MemoryFileSystem>::clone(&vault),
            Arc::<MemoryHostFiles>::clone(&host),
            history,
            trash,
            PathBuf::from("/vault"),
        );
        Self {
            vault,
            host,
            clock,
            files,
        }
    }

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("a test path is valid")
    }

    fn write(&self, path: &str, contents: &str) {
        self.vault.insert(path, contents.as_bytes());
    }

    fn names(&self) -> Vec<String> {
        self.files
            .list_directory(&RelativePath::root())
            .into_iter()
            .map(|entry| entry.name)
            .collect()
    }
}

#[test]
fn listing_sorts_folders_first_and_skips_hidden_entries() {
    let fixture = Fixture::new();
    fixture.write("zulu.md", "z");
    fixture.write("Alpha.md", "a");
    fixture.write(".hidden.md", "h");
    fixture.write(".margin/secret.md", "s");
    fixture.write("notes/one.md", "one");

    assert_eq!(fixture.names(), vec!["notes", "Alpha.md", "zulu.md"]);
    let entries = fixture.files.list_directory(&RelativePath::root());
    assert!(entries[0].is_dir, "folders come first");
    assert_eq!(entries[1].path.as_str(), "Alpha.md");
}

#[test]
fn listing_a_missing_directory_is_empty() {
    let fixture = Fixture::new();
    fixture.write("note.md", "body");

    assert!(
        fixture
            .files
            .list_directory(&Fixture::path("gone"))
            .is_empty()
    );
    assert!(
        fixture
            .files
            .list_directory(&Fixture::path("note.md"))
            .is_empty()
    );
}

#[test]
fn walking_reports_every_entry_and_honours_hidden_entries() {
    let fixture = Fixture::new();
    fixture.write("top.md", "t");
    fixture.write("notes/deep/leaf.md", "l");
    fixture.write(".margin/secret.md", "s");

    let visible: Vec<String> = fixture
        .files
        .walk_directory(&RelativePath::root(), false)
        .into_iter()
        .map(|entry| entry.path.to_string())
        .collect();
    assert_eq!(
        visible,
        vec!["notes", "notes/deep", "notes/deep/leaf.md", "top.md"]
    );

    assert!(
        fixture
            .files
            .walk_directory(&RelativePath::root(), true)
            .iter()
            .any(|entry| entry.path.as_str() == ".margin/secret.md")
    );
}

#[test]
fn a_hidden_folder_is_omitted_from_the_tree_but_stays_on_disk() {
    let fixture = Fixture::new();
    fixture.write("attachments/pic.png", "x");
    fixture.write("notes/one.md", "one");

    let rows = fixture.files.visible_tree(
        &[Fixture::path("attachments")],
        SortBy::Name,
        &[Fixture::path("attachments")],
    );
    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();

    assert_eq!(names, vec!["notes"]);
    assert!(
        fixture.vault.exists(&Fixture::path("attachments/pic.png")),
        "hiding is a presentation rule; the file must still be there"
    );
    assert_eq!(
        fixture
            .files
            .walk_directory(&RelativePath::root(), false)
            .iter()
            .filter(|entry| entry.name == "attachments")
            .count(),
        1,
        "every walker must still see the folder"
    );
}

#[test]
fn nothing_is_hidden_without_a_hidden_entry() {
    let fixture = Fixture::new();
    fixture.write("attachments/pic.png", "x");

    let rows = fixture.files.visible_tree(&[], SortBy::Name, &[]);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "attachments");
    assert!(rows[0].is_dir);
    assert_eq!(rows[0].depth, 0);
}

#[test]
fn the_tree_expands_only_the_open_folders() {
    let fixture = Fixture::new();
    fixture.write("a.md", "a");
    fixture.write("notes/b.md", "b");
    fixture.write("notes/deep/c.md", "c");

    let collapsed = fixture.files.visible_tree(&[], SortBy::Name, &[]);
    let rows: Vec<(String, usize)> = collapsed
        .iter()
        .map(|row| (row.name.clone(), row.depth))
        .collect();
    assert_eq!(
        rows,
        vec![(String::from("notes"), 0), (String::from("a.md"), 0)]
    );

    let expanded = fixture
        .files
        .visible_tree(&[Fixture::path("notes")], SortBy::Name, &[]);
    assert_eq!(
        expanded
            .iter()
            .map(|row| (row.name.clone(), row.depth))
            .collect::<Vec<_>>(),
        vec![
            (String::from("notes"), 0),
            (String::from("deep"), 1),
            (String::from("b.md"), 1),
            (String::from("a.md"), 0),
        ]
    );

    let subtree = fixture.files.subtree(
        &Fixture::path("notes"),
        5,
        &[Fixture::path("notes/deep")],
        SortBy::Name,
        &[],
    );
    assert_eq!(
        subtree
            .iter()
            .map(|row| (row.name.clone(), row.depth))
            .collect::<Vec<_>>(),
        vec![
            (String::from("deep"), 5),
            (String::from("c.md"), 6),
            (String::from("b.md"), 5),
        ]
    );
}

#[test]
fn date_sort_puts_recent_files_first_and_folders_before_them() {
    let fixture = Fixture::new();
    fixture.write("older.md", "o");
    fixture.clock.advance(Duration::from_secs(10));
    fixture.write("newer.md", "n");
    fixture.clock.advance(Duration::from_secs(10));
    fixture.write("zulu/inside.md", "i");

    let names: Vec<String> = fixture
        .files
        .visible_tree(&[], SortBy::Date, &[])
        .into_iter()
        .map(|row| row.name)
        .collect();

    assert_eq!(names, vec!["zulu", "newer.md", "older.md"]);
}

#[test]
fn export_excludes_hidden_paths_and_keeps_relative_names() {
    let fixture = Fixture::new();
    fixture.write("top.md", "A");
    fixture.write("notes/a.md", "B");
    fixture.write("notes/deep/b.md", "C");
    fixture.write(".margin/secret.md", "D");
    fixture.write(".hidden.md", "E");
    fixture.write("notes/.draft.md", "F");
    fixture
        .vault
        .create_dir_all(&Fixture::path("empty"))
        .expect("the folder exists");

    fixture
        .files
        .export_vault_zip(Path::new("/out/vault.zip"))
        .expect("the vault is exported");

    let bytes = fixture
        .host
        .contents(Path::new("/out/vault.zip"))
        .expect("the archive is written");
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("the archive is readable");
    let mut names = archive
        .file_names()
        .map(|name| name.expect("the entry name decodes").into_owned())
        .collect::<Vec<String>>();
    names.sort();
    assert_eq!(
        names,
        vec![
            String::from("empty/"),
            String::from("notes/"),
            String::from("notes/a.md"),
            String::from("notes/deep/"),
            String::from("notes/deep/b.md"),
            String::from("top.md"),
        ]
    );

    let mut contents = String::new();
    archive
        .by_name("notes/a.md")
        .expect("the entry exists")
        .read_to_string(&mut contents)
        .expect("the entry reads");
    assert_eq!(contents, "B");
}
