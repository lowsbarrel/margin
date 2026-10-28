use std::path::PathBuf;
use std::sync::Arc;

use margin_files::{Error, FilesService, MAX_RESULTS};
use margin_kernel::{FileSystem, HistoryStore, RelativePath, StoreError, UnixMillis};
use margin_test_support::{FixedClock, MemoryFileSystem, MemoryHistoryStore, MemoryHostFiles};
use margin_trash::TrashService;

const NOW: u64 = 1_700_000_000_123;

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

    fn texts(&self, path: &str) -> Option<String> {
        self.vault
            .contents(path)
            .map(|bytes| String::from_utf8(bytes).expect("test content is text"))
    }
}

#[test]
fn search_finds_a_name_prefix_and_an_interior_token() {
    let fixture = Fixture::new();
    fixture.write("my-project.md", "one");
    fixture.write("readme.md", "two");
    fixture.write("notes/other.md", "three");

    let prefix: Vec<String> = fixture
        .files
        .search_files("my")
        .into_iter()
        .map(|entry| entry.path.to_string())
        .collect();
    assert_eq!(prefix, vec!["my-project.md"]);

    let token: Vec<String> = fixture
        .files
        .search_files("project")
        .into_iter()
        .map(|entry| entry.path.to_string())
        .collect();
    assert_eq!(token, vec!["my-project.md"]);

    assert!(fixture.files.search_files("zzz").is_empty());
    assert!(fixture.files.search_files("   ").is_empty());
}

#[test]
fn search_ranks_a_prefix_above_a_token_above_a_substring_above_a_path() {
    let fixture = Fixture::new();
    fixture.write("project.md", "a");
    fixture.write("my-project.md", "b");
    fixture.write("apropos.md", "c");
    fixture.write("projects/other.md", "d");

    let paths: Vec<String> = fixture
        .files
        .search_files("pro")
        .into_iter()
        .map(|entry| entry.path.to_string())
        .collect();

    assert_eq!(paths[0], "project.md");
    assert_eq!(paths.last().map(String::as_str), Some("projects/other.md"));
    assert!(paths.contains(&String::from("my-project.md")), "{paths:?}");
    assert!(paths.contains(&String::from("apropos.md")), "{paths:?}");
}

#[test]
fn search_ignores_case_and_reports_files_only() {
    let fixture = Fixture::new();
    fixture.write("Notes/README.md", "one");
    fixture
        .vault
        .create_dir_all(&Fixture::path("proj"))
        .expect("the folder exists");

    let hits = fixture.files.search_files("readme");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path.as_str(), "Notes/README.md");
    assert!(!hits[0].is_dir);
}

#[test]
fn search_trusts_its_token_index_only_until_the_page_is_full() {
    let fixture = Fixture::new();
    fixture.write("projects/other.md", "d");
    for index in 0..3 {
        fixture.write(&format!("notes/x-pro-{index}.md"), "x");
    }

    let hits: Vec<String> = fixture
        .files
        .search_files("pro")
        .into_iter()
        .map(|entry| entry.path.to_string())
        .collect();
    assert!(
        hits.contains(&String::from("projects/other.md")),
        "a short result page falls back to the full scan: {hits:?}"
    );

    let full = Fixture::new();
    full.write("projects/other.md", "d");
    for index in 0..MAX_RESULTS {
        full.write(&format!("notes/x-pro-{index}.md"), "x");
    }
    let hits = full.files.search_files("pro");

    assert_eq!(hits.len(), MAX_RESULTS);
    assert!(
        !hits
            .iter()
            .any(|entry| entry.path.as_str() == "projects/other.md"),
        "a full page of token matches is trusted"
    );
}

#[test]
fn search_rebuilds_after_a_change_and_can_be_invalidated() {
    let fixture = Fixture::new();
    fixture.write("first.md", "one");
    assert_eq!(fixture.files.search_files("first").len(), 1);

    fixture
        .files
        .write_file(&Fixture::path("second.md"), b"two")
        .expect("the file is written");
    assert_eq!(fixture.files.search_files("second").len(), 1);

    fixture.write("third.md", "three");
    assert!(
        fixture.files.search_files("third").is_empty(),
        "the cache holds the old listing"
    );
    fixture.files.invalidate_search_cache();
    assert_eq!(fixture.files.search_files("third").len(), 1);
}

#[test]
fn replace_in_file_replaces_case_sensitively_and_counts() {
    let fixture = Fixture::new();
    fixture.write("note.md", "Cat cat CAT");

    let count = fixture
        .files
        .replace_in_file(&Fixture::path("note.md"), "cat", "dog", true)
        .expect("the file is replaced");

    assert_eq!(count, 1);
    assert_eq!(fixture.texts("note.md").as_deref(), Some("Cat dog CAT"));
}

#[test]
fn replace_in_file_folds_ascii_case_without_moving_offsets() {
    let fixture = Fixture::new();
    fixture.write("note.md", "café CAT cat");

    let count = fixture
        .files
        .replace_in_file(&Fixture::path("note.md"), "cat", "x", false)
        .expect("the file is replaced");

    assert_eq!(count, 2);
    assert_eq!(fixture.texts("note.md").as_deref(), Some("café x x"));
}

#[test]
fn replace_in_file_leaves_a_file_without_matches_untouched() {
    let fixture = Fixture::new();
    fixture.write("note.md", "nothing to see");

    let count = fixture
        .files
        .replace_in_file(&Fixture::path("note.md"), "zzz", "x", false)
        .expect("no match is not an error");

    assert_eq!(count, 0);
    assert_eq!(fixture.texts("note.md").as_deref(), Some("nothing to see"));
    assert!(matches!(
        fixture
            .files
            .replace_in_file(&Fixture::path("gone.md"), "a", "b", true),
        Err(Error::Store(StoreError::NotFound(_)))
    ));
}

#[test]
fn replace_in_file_refuses_a_file_that_is_not_text() {
    let fixture = Fixture::new();
    fixture.vault.insert("blob.md", &[0xff, 0xfe, 0x00]);

    assert!(matches!(
        fixture
            .files
            .replace_in_file(&Fixture::path("blob.md"), "a", "b", true),
        Err(Error::NotText { .. })
    ));
}
