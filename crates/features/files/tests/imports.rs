use std::path::{Path, PathBuf};
use std::sync::Arc;

use margin_files::{Error, FilesService};
use margin_kernel::{FileSystem, HistoryStore, RelativePath, UnixMillis};
use margin_test_support::{FixedClock, MemoryFileSystem, MemoryHistoryStore, MemoryHostFiles};
use margin_trash::TrashService;

const NOW: u64 = 1_700_000_000_123;

struct Fixture {
    vault: Arc<MemoryFileSystem>,
    host: Arc<MemoryHostFiles>,
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
            Arc::<MemoryHostFiles>::clone(&host),
            history,
            trash,
            PathBuf::from("/vault"),
        );
        Self { vault, host, files }
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
fn importing_an_external_file_refuses_an_existing_destination() {
    let fixture = Fixture::new();
    fixture
        .host
        .insert(Path::new("/outside/photo.png"), b"imported");
    fixture.write("existing.png", "keep");

    let occupied = RelativePath::new("existing.png").expect("a valid path");
    let error = fixture
        .files
        .import_file(Path::new("/outside/photo.png"), &occupied)
        .expect_err("the destination is occupied");

    assert!(
        error.to_string().contains("existing.png"),
        "error must name the target: {error}"
    );
    assert_eq!(fixture.text("existing.png").as_deref(), Some("keep"));

    let free = RelativePath::new("nested/photo.png").expect("a valid path");
    fixture
        .files
        .import_file(Path::new("/outside/photo.png"), &free)
        .expect("the destination is free");
    assert_eq!(
        fixture.text("nested/photo.png").as_deref(),
        Some("imported")
    );
}

#[test]
fn importing_an_external_directory_copies_its_contents() {
    let fixture = Fixture::new();
    fixture.host.insert(Path::new("/external/note.md"), b"one");
    fixture
        .host
        .insert(Path::new("/external/nested/deep.md"), b"two");

    fixture
        .files
        .import_directory(Path::new("/external"), &Fixture::path("imported"))
        .expect("the directory is imported");

    assert_eq!(fixture.text("imported/note.md").as_deref(), Some("one"));
    assert_eq!(
        fixture.text("imported/nested/deep.md").as_deref(),
        Some("two")
    );
}

#[test]
fn importing_a_directory_onto_an_existing_one_is_refused() {
    let fixture = Fixture::new();
    fixture.host.insert(Path::new("/external/a.md"), b"a");
    fixture.write("occupied/keep.md", "keep");

    assert!(matches!(
        fixture
            .files
            .import_directory(Path::new("/external"), &Fixture::path("occupied")),
        Err(Error::Occupied { .. })
    ));
    assert_eq!(fixture.text("occupied/keep.md").as_deref(), Some("keep"));
    assert!(!fixture.exists("occupied/a.md"));
}

#[test]
fn importing_a_file_as_a_directory_is_refused() {
    let fixture = Fixture::new();
    fixture.host.insert(Path::new("/external/note.md"), b"body");

    assert!(matches!(
        fixture
            .files
            .import_directory(Path::new("/external/note.md"), &Fixture::path("note.md")),
        Err(Error::NotADirectory(_))
    ));
    assert!(!fixture.exists("note.md"));
}

#[test]
fn importing_a_directory_that_contains_the_vault_is_refused() {
    let fixture = Fixture::new();
    fixture.host.insert(Path::new("/vault/note.md"), b"inside");

    fixture
        .files
        .import_directory(Path::new("/vault"), &Fixture::path("copy"))
        .expect_err("the vault cannot be imported into itself");
    assert!(
        fixture
            .files
            .import_directory(Path::new("/"), &Fixture::path("copy"))
            .is_err(),
        "a source above the vault would copy the vault into itself"
    );
    assert!(!fixture.exists("copy/note.md"));
}
