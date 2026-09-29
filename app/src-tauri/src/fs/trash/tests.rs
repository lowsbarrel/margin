use super::*;
use std::fs;
use std::path::PathBuf;

fn temp_vault(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "margin-trash-test-{}-{tag}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn only_trash_item(vault: &Path) -> PathBuf {
    let items: Vec<PathBuf> = fs::read_dir(vault.join(TRASH_ROOT))
        .expect("trash directory should exist")
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(items.len(), 1, "expected exactly one trash item: {items:?}");
    items.into_iter().next().unwrap()
}

#[test]
fn deleting_a_file_moves_it_and_its_history_into_trash() {
    let vault = temp_vault("file");
    let root = vault.to_string_lossy().into_owned();
    let note = vault.join("notes").join("todo.md");
    fs::create_dir_all(note.parent().unwrap()).unwrap();
    fs::write(&note, "hello").unwrap();
    let snapshot = vault.join(".margin/history/notes/todo.md/1712928000.md");
    fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
    fs::write(&snapshot, "older").unwrap();

    trash_entry(&root, &note.canonicalize().unwrap()).unwrap();

    assert!(!note.exists(), "the original must leave the vault");
    assert!(!snapshot.exists(), "history must travel with the file");
    let item = only_trash_item(&vault);
    assert_eq!(
        fs::read_to_string(item.join("files/notes/todo.md")).unwrap(),
        "hello"
    );
    assert_eq!(
        fs::read_to_string(item.join("history/notes/todo.md/1712928000.md")).unwrap(),
        "older"
    );
    let id = item.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        id_timestamp_millis(&id).is_some_and(|ms| ms > 1_700_000_000_000),
        "id {id} should start with unix milliseconds"
    );

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn deleting_a_directory_moves_the_whole_subtree_and_history() {
    let vault = temp_vault("dir");
    let root = vault.to_string_lossy().into_owned();
    let folder = vault.join("notes");
    let note = folder.join("sub").join("todo.md");
    fs::create_dir_all(note.parent().unwrap()).unwrap();
    fs::write(&note, "nested").unwrap();
    let snapshot = vault.join(".margin/history/notes/sub/todo.md/1712928000.md");
    fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
    fs::write(&snapshot, "older").unwrap();

    trash_entry(&root, &folder.canonicalize().unwrap()).unwrap();

    assert!(!folder.exists());
    let item = only_trash_item(&vault);
    assert_eq!(
        fs::read_to_string(item.join("files/notes/sub/todo.md")).unwrap(),
        "nested"
    );
    assert_eq!(
        fs::read_to_string(item.join("history/notes/sub/todo.md/1712928000.md")).unwrap(),
        "older"
    );

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn deleting_without_history_trashes_only_the_entry() {
    let vault = temp_vault("nohistory");
    let root = vault.to_string_lossy().into_owned();
    let note = vault.join("lonely.md");
    fs::write(&note, "body").unwrap();

    trash_entry(&root, &note.canonicalize().unwrap()).unwrap();

    let item = only_trash_item(&vault);
    assert_eq!(
        fs::read_to_string(item.join("files/lonely.md")).unwrap(),
        "body"
    );
    assert!(!item.join("history").exists());

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn trash_ids_do_not_collide() {
    let vault = temp_vault("ids");
    let trash_root = vault.join(TRASH_ROOT);
    fs::create_dir_all(&trash_root).unwrap();

    let mut seen = Vec::new();
    for _ in 0..8 {
        let item = new_trash_item(&trash_root);
        assert!(!seen.contains(&item), "{item:?} was handed out twice");
        fs::create_dir_all(&item).unwrap();
        seen.push(item);
    }

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn purge_removes_only_items_past_the_age() {
    let vault = temp_vault("purge");
    let root = vault.to_string_lossy().into_owned();
    let trash_root = vault.join(TRASH_ROOT);
    let now = now_millis();
    let expired = trash_root.join(format!("{}-0", now - PURGE_AGE_MS - 1_000));
    let fresh = trash_root.join(format!("{}-1", now - 1_000));
    let foreign = trash_root.join("not-an-id");
    for dir in [&expired, &fresh, &foreign] {
        fs::create_dir_all(dir.join(FILES_DIR)).unwrap();
    }

    assert_eq!(purge_older_than(&root, PURGE_AGE_MS), 1);

    assert!(!expired.exists(), "an expired item should be gone");
    assert!(fresh.exists(), "a recent item must survive");
    assert!(foreign.exists(), "an unrecognized item must be left alone");

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn purge_on_an_empty_vault_is_a_no_op() {
    let vault = temp_vault("purge-empty");
    assert_eq!(purge_older_than(&vault.to_string_lossy(), PURGE_AGE_MS), 0);
    fs::remove_dir_all(&vault).ok();
}

#[test]
fn listing_reports_the_original_path_of_a_deleted_folder() {
    let vault = temp_vault("list-dir");
    let root = vault.to_string_lossy().into_owned();
    let folder = vault.join("notes/sub");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("todo.md"), "kept").unwrap();
    fs::write(folder.join("other.md"), "also kept").unwrap();

    trash_entry(&root, &folder.canonicalize().unwrap()).unwrap();

    let items = list_items(&root);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, "notes/sub");
    assert_eq!(items[0].name, "sub");
    assert!(items[0].is_dir);
    assert!(!items[0].has_history);
    assert!(items[0].deleted_at > 1_700_000_000_000.0);

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn restoring_puts_the_entry_and_its_history_back() {
    let vault = temp_vault("restore");
    let root = vault.to_string_lossy().into_owned();
    let note = vault.join("notes/todo.md");
    fs::create_dir_all(note.parent().unwrap()).unwrap();
    fs::write(&note, "hello").unwrap();
    let snapshot = vault.join(".margin/history/notes/todo.md/1712928000123.md");
    fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
    fs::write(&snapshot, "older").unwrap();

    trash_entry(&root, &note.canonicalize().unwrap()).unwrap();
    let id = list_items(&root)[0].id.clone();

    let restored = restore(&root, &id).unwrap();

    assert_eq!(restored, "notes/todo.md");
    assert_eq!(fs::read_to_string(&note).unwrap(), "hello");
    assert_eq!(fs::read_to_string(&snapshot).unwrap(), "older");
    assert!(list_items(&root).is_empty(), "the item is consumed");
    assert!(
        !vault.join(TRASH_ROOT).join(&id).exists(),
        "the emptied item directory is cleaned up"
    );

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn restoring_never_replaces_what_occupies_the_original_path() {
    let vault = temp_vault("collision");
    let root = vault.to_string_lossy().into_owned();
    let note = vault.join("notes/Todo.md");
    fs::create_dir_all(note.parent().unwrap()).unwrap();
    fs::write(&note, "deleted").unwrap();
    trash_entry(&root, &note.canonicalize().unwrap()).unwrap();

    fs::write(&note, "occupant").unwrap();
    let id = list_items(&root)[0].id.clone();

    let restored = restore(&root, &id).unwrap();

    assert_eq!(restored, "notes/Todo (restored).md");
    assert_eq!(fs::read_to_string(&note).unwrap(), "occupant");
    assert_eq!(
        fs::read_to_string(vault.join("notes/Todo (restored).md")).unwrap(),
        "deleted"
    );

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn restoring_merges_history_into_an_existing_history_directory() {
    let vault = temp_vault("restore-history-merge");
    let root = vault.to_string_lossy().into_owned();
    let note = vault.join("todo.md");
    fs::write(&note, "deleted").unwrap();
    let old = vault.join(".margin/history/todo.md/1712928000123.md");
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::write(&old, "from trash").unwrap();

    trash_entry(&root, &note.canonicalize().unwrap()).unwrap();
    fs::write(&note, "occupant").unwrap();
    let fresh = vault.join(".margin/history/todo.md/1712928000124.md");
    fs::create_dir_all(fresh.parent().unwrap()).unwrap();
    fs::write(&fresh, "from vault").unwrap();
    let id = list_items(&root)[0].id.clone();

    restore(&root, &id).unwrap();

    assert_eq!(fs::read_to_string(&fresh).unwrap(), "from vault");
    let restored_history = vault.join(".margin/history/todo (restored).md");
    let entries: Vec<String> = fs::read_dir(&restored_history)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries.len(), 1, "the trashed history came along");
    assert_eq!(
        fs::read_to_string(restored_history.join(&entries[0])).unwrap(),
        "from trash"
    );

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn a_trash_item_without_the_recorded_path_still_restores() {
    let vault = temp_vault("legacy");
    let root = vault.to_string_lossy().into_owned();
    let item = vault.join(TRASH_ROOT).join("1700000000000-0");
    fs::create_dir_all(item.join("files/notes")).unwrap();
    fs::write(item.join("files/notes/todo.md"), "legacy").unwrap();

    assert_eq!(list_items(&root)[0].path, "notes/todo.md");

    assert_eq!(restore(&root, "1700000000000-0").unwrap(), "notes/todo.md");
    assert_eq!(
        fs::read_to_string(vault.join("notes/todo.md")).unwrap(),
        "legacy"
    );

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn delete_removes_one_item_and_empty_removes_the_rest() {
    let vault = temp_vault("delete-empty");
    let root = vault.to_string_lossy().into_owned();
    for name in ["a.md", "b.md", "c.md"] {
        fs::write(vault.join(name), name).unwrap();
        trash_entry(&root, &vault.join(name).canonicalize().unwrap()).unwrap();
    }

    let ids: Vec<String> = list_items(&root).into_iter().map(|i| i.id).collect();
    assert_eq!(ids.len(), 3);
    delete(&root, &ids[0]).unwrap();
    assert_eq!(list_items(&root).len(), 2);

    assert_eq!(empty(&root), 2);
    assert!(list_items(&root).is_empty());

    fs::remove_dir_all(&vault).ok();
}

#[test]
fn trash_commands_refuse_an_id_that_escapes_the_trash() {
    let vault = temp_vault("bad-id");
    let root = vault.to_string_lossy().into_owned();
    fs::create_dir_all(vault.join("notes")).unwrap();
    fs::write(vault.join("notes/keep.md"), "keep").unwrap();

    assert!(delete(&root, "../notes").is_err());
    assert!(restore(&root, "not-an-id").is_err());
    assert!(vault.join("notes/keep.md").exists());

    fs::remove_dir_all(&vault).ok();
}
