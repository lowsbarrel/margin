use std::path::PathBuf;
use std::sync::Arc;

use margin_files::{Error, FilesService, MAX_WALK_DEPTH};
use margin_kernel::{EntryKind, FileSystem, HistoryStore, RelativePath, StoreError, UnixMillis};
use margin_test_support::{FixedClock, MemoryFileSystem, MemoryHistoryStore, MemoryHostFiles};
use margin_trash::TrashService;

const NOW: u64 = 1_700_000_000_123;
const NOTES: &str = "notes/deep";

struct Fixture {
    vault: Arc<MemoryFileSystem>,
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
            clock,
        ));
        let files = FilesService::new(
            Arc::<MemoryFileSystem>::clone(&vault),
            host,
            history,
            trash,
            PathBuf::from("/vault"),
        );
        Self { vault, files }
    }

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("a test path is valid")
    }

    fn write(&self, path: &str, contents: &str) {
        self.vault.insert(path, contents.as_bytes());
    }

    fn text(&self, path: &str) -> Option<String> {
        self.vault
            .contents(path)
            .map(|bytes| String::from_utf8(bytes).expect("test content is text"))
    }

    fn exists(&self, path: &str) -> bool {
        self.vault.exists(&Self::path(path))
    }
}

#[test]
fn rename_refuses_an_existing_destination() {
    let fixture = Fixture::new();
    fixture.write("from.md", "source");
    fixture.write("to.md", "target");

    let error = fixture
        .files
        .rename_entry(&Fixture::path("from.md"), &Fixture::path("to.md"))
        .expect_err("the destination is occupied");

    assert!(
        error.to_string().contains("to.md"),
        "error must name the target: {error}"
    );
    assert_eq!(fixture.text("to.md").as_deref(), Some("target"));
    assert_eq!(fixture.text("from.md").as_deref(), Some("source"));
}

#[test]
fn rename_into_a_free_path_still_renames() {
    let fixture = Fixture::new();
    fixture.write("from.md", "source");

    fixture
        .files
        .rename_entry(&Fixture::path("from.md"), &Fixture::path("nested/to.md"))
        .expect("the destination is free");

    assert!(!fixture.exists("from.md"));
    assert_eq!(fixture.text("nested/to.md").as_deref(), Some("source"));
}

#[test]
fn case_only_rename_of_the_same_file_changes_its_spelling() {
    let fixture = Fixture::new();
    fixture.write("note.md", "body");

    fixture
        .files
        .rename_entry(&Fixture::path("note.md"), &Fixture::path("Note.md"))
        .expect("a case-only rename stays the same entry");

    assert_eq!(fixture.text("Note.md").as_deref(), Some("body"));
    assert!(!fixture.exists("note.md"));
}

#[test]
fn case_varied_destination_is_a_conflict_when_both_spellings_exist() {
    let fixture = Fixture::new();
    fixture.write("note.md", "lower");
    fixture.write("Note.md", "upper");

    assert!(matches!(
        fixture
            .files
            .rename_entry(&Fixture::path("note.md"), &Fixture::path("Note.md")),
        Err(Error::Occupied { .. })
    ));
    assert_eq!(fixture.text("Note.md").as_deref(), Some("upper"));
    assert_eq!(fixture.text("note.md").as_deref(), Some("lower"));
}

#[test]
fn renaming_a_note_moves_its_history_with_it() {
    let fixture = Fixture::new();
    fixture.write("notes/todo.md", "body");
    fixture.write(".margin/history/notes/todo.md/1712928000123.md", "older");

    fixture
        .files
        .rename_entry(
            &Fixture::path("notes/todo.md"),
            &Fixture::path("notes/done.md"),
        )
        .expect("the note is renamed");

    assert_eq!(fixture.text("notes/done.md").as_deref(), Some("body"));
    assert_eq!(
        fixture
            .text(".margin/history/notes/done.md/1712928000123.md")
            .as_deref(),
        Some("older")
    );
    assert!(!fixture.exists(".margin/history/notes/todo.md"));
}

#[test]
fn copying_a_file_onto_an_existing_one_is_refused() {
    let fixture = Fixture::new();
    fixture.write("inside.md", "source");
    fixture.write("target.md", "target");

    let error = fixture
        .files
        .copy_file(&Fixture::path("inside.md"), &Fixture::path("target.md"))
        .expect_err("the destination is occupied");

    assert!(
        error.to_string().contains("target.md"),
        "error must name the target: {error}"
    );
    assert_eq!(fixture.text("target.md").as_deref(), Some("target"));
}

#[test]
fn copying_a_file_into_a_free_path_copies_it() {
    let fixture = Fixture::new();
    fixture.write("inside.md", "source");

    fixture
        .files
        .copy_file(
            &Fixture::path("inside.md"),
            &Fixture::path("nested/copy.md"),
        )
        .expect("the destination is free");

    assert_eq!(fixture.text("nested/copy.md").as_deref(), Some("source"));
    assert_eq!(fixture.text("inside.md").as_deref(), Some("source"));
}

#[test]
fn copying_a_directory_onto_an_existing_one_is_refused() {
    let fixture = Fixture::new();
    fixture.write("src/sub/a.md", "a");
    fixture.write("occupied/keep.md", "keep");

    assert!(matches!(
        fixture
            .files
            .copy_directory(&Fixture::path("src"), &Fixture::path("occupied")),
        Err(Error::Occupied { .. })
    ));
    assert_eq!(fixture.text("occupied/keep.md").as_deref(), Some("keep"));
    assert!(!fixture.exists("occupied/sub/a.md"));

    fixture
        .files
        .copy_directory(&Fixture::path("src"), &Fixture::path("free"))
        .expect("the destination is free");
    assert_eq!(fixture.text("free/sub/a.md").as_deref(), Some("a"));
}

#[test]
fn copying_a_directory_into_itself_is_refused() {
    let fixture = Fixture::new();
    fixture.write("src/a.md", "a");

    assert!(matches!(
        fixture
            .files
            .copy_directory(&Fixture::path("src"), &Fixture::path("src/inner")),
        Err(Error::NestedDestination)
    ));
    assert!(!fixture.exists("src/inner/a.md"));
}

#[test]
fn copying_a_tree_deeper_than_the_walk_limit_is_refused() {
    let fixture = Fixture::new();
    let deep = (0..=MAX_WALK_DEPTH).fold(String::from("src"), |path, step| {
        format!("{path}/level{step}")
    });
    fixture.write(&format!("{deep}/a.md"), "a");

    fixture
        .files
        .copy_directory(&Fixture::path("src"), &Fixture::path("copy"))
        .expect_err("a tree past the walk limit is not copied");
}

#[test]
fn deleting_an_entry_moves_it_into_the_trash() {
    let fixture = Fixture::new();
    fixture.write("notes/todo.md", "body");
    fixture.write(".margin/history/notes/todo.md/1712928000123.md", "older");

    fixture
        .files
        .delete_entry(&Fixture::path("notes/todo.md"))
        .expect("the entry moves into the trash");

    assert!(!fixture.exists("notes/todo.md"));
    let items = fixture
        .vault
        .list(&Fixture::path(".margin/trash"))
        .expect("the trash directory exists");
    assert_eq!(items.len(), 1);
    let item = items[0].name.clone();
    assert_eq!(
        fixture
            .text(&format!(".margin/trash/{item}/files/notes/todo.md"))
            .as_deref(),
        Some("body")
    );
}

#[test]
fn create_directory_makes_the_whole_path() {
    let fixture = Fixture::new();

    fixture
        .files
        .create_directory(&Fixture::path(NOTES))
        .expect("the directory is created");

    assert!(fixture.exists(NOTES));
    assert_eq!(
        fixture
            .vault
            .metadata(&Fixture::path(NOTES))
            .expect("the directory exists")
            .kind,
        EntryKind::Directory
    );
}

#[test]
fn metadata_reports_size_and_a_missing_file_says_so() {
    let fixture = Fixture::new();
    fixture.write("note.md", "body");

    assert!(fixture.files.exists(&Fixture::path("note.md")));
    let metadata = fixture
        .files
        .metadata(&Fixture::path("note.md"))
        .expect("the file exists");
    assert_eq!(metadata.size, 4);
    assert_eq!(metadata.modified, UnixMillis::from_millis(NOW));

    let missing = fixture.files.metadata(&Fixture::path("gone.md"));
    assert!(matches!(
        missing,
        Err(Error::Store(StoreError::NotFound(_)))
    ));
}

#[test]
fn setting_the_mtime_keeps_it() {
    let fixture = Fixture::new();
    fixture.write("note.md", "body");
    let stamp = UnixMillis::from_millis(1_600_000_000_000);

    fixture
        .files
        .set_modified(&Fixture::path("note.md"), stamp)
        .expect("the mtime is set");

    let metadata = fixture
        .files
        .metadata(&Fixture::path("note.md"))
        .expect("the file exists");
    assert_eq!(metadata.modified, stamp);
    assert!(matches!(
        fixture.files.set_modified(&Fixture::path("gone.md"), stamp),
        Err(Error::Store(StoreError::NotFound(_)))
    ));
}

#[test]
fn writing_and_reading_a_file_round_trips() {
    let fixture = Fixture::new();

    fixture
        .files
        .write_file(&Fixture::path("notes/new.md"), b"body")
        .expect("the file is written");

    assert_eq!(
        fixture
            .files
            .read_file(&Fixture::path("notes/new.md"))
            .expect("the file reads"),
        b"body"
    );
    assert!(matches!(
        fixture.files.read_file(&Fixture::path("gone.md")),
        Err(Error::Store(StoreError::NotFound(_)))
    ));
}
