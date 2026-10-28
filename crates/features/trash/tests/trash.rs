use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use margin_kernel::{FileSystem, HistoryStore, PageRequest, RelativePath, UnixMillis};
use margin_test_support::{FixedClock, MemoryFileSystem, MemoryHistoryStore};
use margin_trash::{Error, PURGE_AGE, TrashItem, TrashService};

const NOW: u64 = 1_700_000_000_123;
const AGE_MILLIS: u64 = 30 * 24 * 60 * 60 * 1000;
const TRASH: &str = ".margin/trash";
const HISTORY: &str = ".margin/history";

struct Fixture {
    vault: Arc<MemoryFileSystem>,
    clock: Arc<FixedClock>,
    trash: TrashService,
}

impl Fixture {
    fn new() -> Self {
        let clock = Arc::new(FixedClock::new(UnixMillis::from_millis(NOW)));
        let vault = Arc::new(MemoryFileSystem::with_clock(Arc::clone(&clock)));
        let history: Arc<dyn HistoryStore> = Arc::new(MemoryHistoryStore::new(Arc::clone(&vault)));
        let trash = TrashService::new(
            Arc::<MemoryFileSystem>::clone(&vault),
            history,
            Arc::<FixedClock>::clone(&clock),
        );
        Self {
            vault,
            clock,
            trash,
        }
    }

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("a test path is valid")
    }

    fn write(&self, path: &str, contents: &str) {
        self.vault.insert(path, contents.as_bytes());
    }

    fn exists(&self, path: &str) -> bool {
        self.vault.exists(&Self::path(path))
    }

    fn text(&self, path: &str) -> Option<String> {
        self.vault
            .contents(path)
            .map(|bytes| String::from_utf8(bytes).expect("test content is text"))
    }

    fn item_text(&self, id: &str, path: &str) -> Option<String> {
        self.text(&format!("{TRASH}/{id}/{path}"))
    }

    fn item_exists(&self, id: &str, path: &str) -> bool {
        self.exists(&format!("{TRASH}/{id}/{path}"))
    }

    fn move_to_trash(&self, path: &str) {
        self.trash
            .move_entry(&Self::path(path))
            .expect("the entry moves into the trash");
    }

    fn items(&self) -> Vec<TrashItem> {
        self.trash.list(&PageRequest::first(usize::MAX)).items
    }

    fn item_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .vault
            .list(&Self::path(TRASH))
            .map(|entries| entries.into_iter().map(|entry| entry.name).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    fn only_item(&self) -> String {
        let names = self.item_names();
        assert_eq!(names.len(), 1, "expected exactly one trash item: {names:?}");
        names.into_iter().next().expect("the item name is present")
    }

    fn make_item(&self, id: &str) {
        self.vault
            .create_dir_all(&Self::path(&format!("{TRASH}/{id}/files")))
            .expect("the item directory is created");
    }
}

#[test]
fn deleting_a_file_moves_it_and_its_history_into_trash() {
    let fixture = Fixture::new();
    fixture.write("notes/todo.md", "hello");
    fixture.write(&format!("{HISTORY}/notes/todo.md/1712928000.md"), "older");

    fixture.move_to_trash("notes/todo.md");

    assert!(
        !fixture.exists("notes/todo.md"),
        "the original leaves the vault"
    );
    assert!(
        !fixture.exists(&format!("{HISTORY}/notes/todo.md/1712928000.md")),
        "history travels with the file"
    );
    let id = fixture.only_item();
    assert_eq!(
        fixture.item_text(&id, "files/notes/todo.md").as_deref(),
        Some("hello")
    );
    assert_eq!(
        fixture
            .item_text(&id, "history/notes/todo.md/1712928000.md")
            .as_deref(),
        Some("older")
    );
    assert_eq!(
        fixture.item_text(&id, "path").as_deref(),
        Some("notes/todo.md")
    );
    let stamp: u64 = id
        .split('-')
        .next()
        .and_then(|first| first.parse().ok())
        .expect("the id starts with unix milliseconds");
    assert!(
        stamp > 1_700_000_000_000,
        "id {id} should start with unix milliseconds"
    );
}

#[test]
fn deleting_a_directory_moves_the_whole_subtree_and_history() {
    let fixture = Fixture::new();
    fixture.write("notes/sub/todo.md", "nested");
    fixture.write(
        &format!("{HISTORY}/notes/sub/todo.md/1712928000.md"),
        "older",
    );

    fixture.move_to_trash("notes");

    assert!(!fixture.exists("notes"));
    let id = fixture.only_item();
    assert_eq!(
        fixture.item_text(&id, "files/notes/sub/todo.md").as_deref(),
        Some("nested")
    );
    assert_eq!(
        fixture
            .item_text(&id, "history/notes/sub/todo.md/1712928000.md")
            .as_deref(),
        Some("older")
    );
}

#[test]
fn deleting_without_history_trashes_only_the_entry() {
    let fixture = Fixture::new();
    fixture.write("lonely.md", "body");

    fixture.move_to_trash("lonely.md");

    let id = fixture.only_item();
    assert_eq!(
        fixture.item_text(&id, "files/lonely.md").as_deref(),
        Some("body")
    );
    assert!(!fixture.item_exists(&id, "history"));
}

#[test]
fn trashing_the_vault_root_is_refused() {
    let fixture = Fixture::new();
    fixture.write("notes/todo.md", "body");

    assert!(matches!(
        fixture.trash.move_entry(&RelativePath::root()),
        Err(Error::VaultRoot)
    ));
    assert!(fixture.exists("notes/todo.md"));
    assert!(!fixture.exists(TRASH));
}

#[test]
fn trash_ids_do_not_collide() {
    let fixture = Fixture::new();
    for index in 0..8 {
        let name = format!("note-{index}.md");
        fixture.write(&name, "body");
        fixture.move_to_trash(&name);
    }

    let names = fixture.item_names();
    assert_eq!(names.len(), 8);
    let unique: HashSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), 8, "{names:?} handed out a duplicate id");
}

#[test]
fn purge_removes_only_items_past_the_age() {
    let fixture = Fixture::new();
    assert_eq!(PURGE_AGE, Duration::from_millis(AGE_MILLIS));
    let expired = format!("{}-0", NOW - AGE_MILLIS - 1_000);
    let fresh = format!("{}-1", NOW - 1_000);
    for id in [&expired, &fresh, "not-an-id"] {
        fixture.make_item(id);
    }

    assert_eq!(fixture.trash.purge_older_than(PURGE_AGE), 1);

    assert!(
        !fixture.exists(&format!("{TRASH}/{expired}")),
        "an expired item is gone"
    );
    assert!(
        fixture.exists(&format!("{TRASH}/{fresh}")),
        "a recent item survives"
    );
    assert!(
        fixture.exists(&format!("{TRASH}/not-an-id")),
        "an unrecognized item is left alone"
    );
}

#[test]
fn purge_on_an_empty_vault_is_a_no_op() {
    assert_eq!(Fixture::new().trash.purge_older_than(PURGE_AGE), 0);
}

#[test]
fn purge_expired_uses_the_thirty_day_window() {
    let fixture = Fixture::new();
    fixture.make_item(&format!("{}-0", NOW - AGE_MILLIS - 1));
    assert_eq!(fixture.trash.purge_expired(), 1);
}

#[test]
fn listing_reports_the_original_path_of_a_deleted_folder() {
    let fixture = Fixture::new();
    fixture.write("notes/sub/todo.md", "kept");
    fixture.write("notes/sub/other.md", "also kept");

    fixture.move_to_trash("notes/sub");

    let items = fixture.items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path.as_str(), "notes/sub");
    assert_eq!(items[0].name, "sub");
    assert!(items[0].is_dir);
    assert!(!items[0].has_history);
    assert_eq!(items[0].id, fixture.only_item());
    assert!(items[0].deleted_at > UnixMillis::from_millis(1_700_000_000_000));
}

#[test]
fn listing_sorts_the_newest_deletion_first() {
    let fixture = Fixture::new();
    fixture.write("first.md", "one");
    fixture.move_to_trash("first.md");
    fixture.clock.advance(Duration::from_secs(5));
    fixture.write("second.md", "two");
    fixture.move_to_trash("second.md");

    let items = fixture.items();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "second.md");
    assert_eq!(items[1].name, "first.md");
}

#[test]
fn listing_pages_through_the_items() {
    let fixture = Fixture::new();
    for name in ["a.md", "b.md", "c.md"] {
        fixture.write(name, name);
        fixture.move_to_trash(name);
    }

    let first = fixture.trash.list(&PageRequest::first(2));
    assert_eq!(first.len(), 2);
    let cursor = first.next.expect("a second page follows");
    let second = fixture.trash.list(&PageRequest::after(cursor, 2));
    assert_eq!(second.len(), 1);
    assert!(second.is_last());

    let mut paths: Vec<String> = first
        .items
        .into_iter()
        .chain(second.items)
        .map(|item| item.name)
        .collect();
    paths.sort();
    assert_eq!(paths, vec!["a.md", "b.md", "c.md"]);
}
