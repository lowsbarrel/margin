use std::sync::Arc;

use margin_kernel::{FileSystem, HistoryStore, PageRequest, RelativePath, UnixMillis};
use margin_test_support::{FixedClock, MemoryFileSystem, MemoryHistoryStore};
use margin_trash::{TrashItem, TrashService};

const NOW: u64 = 1_700_000_000_123;
const TRASH: &str = ".margin/trash";
const HISTORY: &str = ".margin/history";

struct Fixture {
    vault: Arc<MemoryFileSystem>,
    trash: TrashService,
}

impl Fixture {
    fn new() -> Self {
        let clock = Arc::new(FixedClock::new(UnixMillis::from_millis(NOW)));
        let vault = Arc::new(MemoryFileSystem::with_clock(Arc::clone(&clock)));
        let history: Arc<dyn HistoryStore> = Arc::new(MemoryHistoryStore::new(Arc::clone(&vault)));
        let trash = TrashService::new(Arc::<MemoryFileSystem>::clone(&vault), history, clock);
        Self { vault, trash }
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

    fn history_text(&self, path: &str) -> Option<String> {
        self.text(&format!("{HISTORY}/{path}"))
    }

    fn move_to_trash(&self, path: &str) {
        self.trash
            .move_entry(&Self::path(path))
            .expect("the entry moves into the trash");
    }

    fn items(&self) -> Vec<TrashItem> {
        self.trash.list(&PageRequest::first(usize::MAX)).items
    }

    fn make_item(&self, id: &str) {
        self.vault
            .create_dir_all(&Self::path(&format!("{TRASH}/{id}/files")))
            .expect("the item directory is created");
    }
}
#[test]
fn restoring_puts_the_entry_and_its_history_back() {
    let fixture = Fixture::new();
    fixture.write("notes/todo.md", "hello");
    fixture.write(
        &format!("{HISTORY}/notes/todo.md/1712928000123.md"),
        "older",
    );

    fixture.move_to_trash("notes/todo.md");
    let id = fixture.items()[0].id.clone();

    let restored = fixture.trash.restore(&id).expect("the item restores");

    assert_eq!(restored.as_str(), "notes/todo.md");
    assert_eq!(fixture.text("notes/todo.md").as_deref(), Some("hello"));
    assert_eq!(
        fixture
            .history_text("notes/todo.md/1712928000123.md")
            .as_deref(),
        Some("older")
    );
    assert!(fixture.items().is_empty(), "the item is consumed");
    assert!(
        !fixture.exists(&format!("{TRASH}/{id}")),
        "the emptied item directory is cleaned up"
    );
}

#[test]
fn restoring_never_replaces_what_occupies_the_original_path() {
    let fixture = Fixture::new();
    fixture.write("notes/Todo.md", "deleted");
    fixture.move_to_trash("notes/Todo.md");

    fixture.write("notes/Todo.md", "occupant");
    let id = fixture.items()[0].id.clone();

    let restored = fixture.trash.restore(&id).expect("the item restores");

    assert_eq!(restored.as_str(), "notes/Todo (restored).md");
    assert_eq!(fixture.text("notes/Todo.md").as_deref(), Some("occupant"));
    assert_eq!(
        fixture.text("notes/Todo (restored).md").as_deref(),
        Some("deleted")
    );
}

#[test]
fn restoring_merges_history_into_an_existing_history_directory() {
    let fixture = Fixture::new();
    fixture.write("todo.md", "deleted");
    fixture.write(&format!("{HISTORY}/todo.md/1712928000123.md"), "from trash");
    fixture.move_to_trash("todo.md");

    fixture.write("todo.md", "occupant");
    fixture.write(&format!("{HISTORY}/todo.md/1712928000124.md"), "from vault");
    let id = fixture.items()[0].id.clone();

    fixture.trash.restore(&id).expect("the item restores");

    assert_eq!(
        fixture.history_text("todo.md/1712928000124.md").as_deref(),
        Some("from vault")
    );
    assert_eq!(
        fixture
            .history_text("todo (restored).md/1712928000123.md")
            .as_deref(),
        Some("from trash"),
        "the trashed history came along"
    );
}

#[test]
fn a_trash_item_without_the_recorded_path_still_restores() {
    let fixture = Fixture::new();
    fixture.write(
        &format!("{TRASH}/1700000000000-0/files/notes/todo.md"),
        "legacy",
    );

    assert_eq!(fixture.items()[0].path.as_str(), "notes/todo.md");

    let restored = fixture
        .trash
        .restore("1700000000000-0")
        .expect("the item restores");
    assert_eq!(restored.as_str(), "notes/todo.md");
    assert_eq!(fixture.text("notes/todo.md").as_deref(), Some("legacy"));
}

#[test]
fn delete_removes_one_item_and_empty_removes_the_rest() {
    let fixture = Fixture::new();
    for name in ["a.md", "b.md", "c.md"] {
        fixture.write(name, name);
        fixture.move_to_trash(name);
    }

    let ids: Vec<String> = fixture.items().into_iter().map(|item| item.id).collect();
    assert_eq!(ids.len(), 3);
    fixture.trash.delete(&ids[0]).expect("the item is deleted");
    assert_eq!(fixture.items().len(), 2);

    assert_eq!(fixture.trash.empty(), 2);
    assert!(fixture.items().is_empty());
}

#[test]
fn trash_commands_refuse_an_id_that_escapes_the_trash() {
    let fixture = Fixture::new();
    fixture.write("notes/keep.md", "keep");

    fixture
        .trash
        .delete("../notes")
        .expect_err("an id outside the trash is refused");
    fixture
        .trash
        .restore("not-an-id")
        .expect_err("a malformed id is refused");
    fixture
        .trash
        .restore("1700000000000-0")
        .expect_err("a missing item is refused");
    assert!(fixture.exists("notes/keep.md"));
}

#[test]
fn an_empty_trash_item_is_not_listed() {
    let fixture = Fixture::new();
    fixture.make_item("1700000000000-0");

    fixture
        .trash
        .restore("1700000000000-0")
        .expect_err("an empty item cannot be restored");
    assert!(
        fixture.items().is_empty(),
        "an item holding nothing is not listed"
    );
}
