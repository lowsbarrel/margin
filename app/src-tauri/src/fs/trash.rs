//! Reversible deletes.
//!
//! `delete_entry` moves an entry here instead of unlinking it, so a mistaken
//! delete survives until the purge age passes:
//!
//! ```text
//! <vault>/.margin/trash/<id>/files/<rel>     the deleted entry
//! <vault>/.margin/trash/<id>/history/<rel>   its snapshot history, if any
//! ```
//!
//! where `<rel>` is the vault-relative path the entry had, and `<id>` =
//! `<unix_millis>-<counter>` so an item's age is derivable from its name alone.

use super::MAX_WALK_DEPTH;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Trash items older than this are purged when a vault is opened.
pub(crate) const PURGE_AGE_MS: u64 = 30 * 24 * 60 * 60 * 1000;

const TRASH_ROOT: &str = ".margin/trash";
const HISTORY_ROOT: &str = ".margin/history";
const FILES_DIR: &str = "files";
const HISTORY_DIR: &str = "history";
/// Sidecar holding the vault-relative path the entry had when it was deleted.
const REL_FILE: &str = "path";

static TRASH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unix milliseconds, 0 when the clock sits before the epoch.
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The deletion time recorded in a trash id (`<unix_millis>-<counter>`).
fn id_timestamp_millis(id: &str) -> Option<u64> {
    id.split('-').next()?.parse().ok()
}

/// A free trash directory, named after the deletion time. The counter is what
/// keeps two deletes in the same millisecond apart; the existence check covers
/// the counter repeating across a restart.
fn new_trash_item(trash_root: &Path) -> PathBuf {
    let millis = now_millis();
    loop {
        let n = TRASH_COUNTER.fetch_add(1, Ordering::Relaxed);
        let item = trash_root.join(format!("{millis}-{n}"));
        if !item.exists() {
            return item;
        }
    }
}

/// The entry's path relative to the vault root.
///
/// `path` comes from `ensure_in_vault`, which **canonicalizes** — on Windows
/// that is the `\\?\C:\…` verbatim form — so it is stripped against the
/// canonical root but re-joined onto the raw root, the spelling the rest of the
/// app reads and writes. Mirrors `index::index_path_string`.
fn vault_relative(root: &str, path: &Path) -> Result<PathBuf, String> {
    let canonical_root = Path::new(root)
        .canonicalize()
        .map_err(|e| format!("Failed to resolve vault root: {e}"))?;
    let rel = path
        .strip_prefix(&canonical_root)
        .map_err(|_| "Path is not inside the vault".to_string())?;
    if rel.as_os_str().is_empty() {
        return Err("Refusing to delete the vault root".into());
    }
    Ok(rel.to_path_buf())
}

/// Move `src` to `dst`, falling back to copy-then-remove when the two live on
/// different devices (`.margin` may be a mount point).
///
/// The fallback copies in full *before* touching the original: a failed copy
/// leaves the entry in place rather than destroying it half-moved.
fn move_entry(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create trash directory: {e}"))?;
    }
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            if src.is_dir() {
                super::copy_dir_recursive(src, dst)?;
                fs::remove_dir_all(src)
                    .map_err(|e| format!("Failed to remove original directory: {e}"))
            } else {
                fs::copy(src, dst).map_err(|e| format!("Failed to copy into trash: {e}"))?;
                fs::remove_file(src).map_err(|e| format!("Failed to remove original file: {e}"))
            }
        }
        Err(e) => Err(format!("Failed to move into trash: {e}")),
    }
}

/// Move `path` and its snapshot history into the vault's trash.
pub(crate) fn trash_entry(root: &str, path: &Path) -> Result<(), String> {
    let root_path = Path::new(root);
    let rel = vault_relative(root, path)?;
    let item = new_trash_item(&root_path.join(TRASH_ROOT));

    move_entry(path, &item.join(FILES_DIR).join(&rel))?;

    // The relative path is recorded because it cannot be recovered from
    // `files/`: the directories above the entry exist there too, and a deleted
    // folder is indistinguishable from the chain leading to it.
    if let Err(e) = fs::write(item.join(REL_FILE), normalise_rel(&rel)) {
        eprintln!("Failed to record trash item path: {e}");
    }

    // History is derived: an entry that is already gone must not be reported as
    // a failed delete because its snapshots could not follow it.
    let history_src = root_path.join(HISTORY_ROOT).join(&rel);
    if history_src.exists()
        && let Err(e) = move_entry(&history_src, &item.join(HISTORY_DIR).join(&rel))
    {
        eprintln!("Failed to move history into trash: {e}");
    }
    Ok(())
}

/// Remove trash items whose deletion time is older than `max_age_ms`.
///
/// Best-effort: unreadable or unrecognized items are left alone, so a future
/// trash format is not destroyed by an older build.
pub(crate) fn purge_older_than(root: &str, max_age_ms: u64) -> u32 {
    let Ok(entries) = fs::read_dir(Path::new(root).join(TRASH_ROOT)) else {
        return 0;
    };
    let now = now_millis();
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(age) = name
            .to_str()
            .and_then(id_timestamp_millis)
            .map(|deleted_at| now.saturating_sub(deleted_at))
        else {
            continue;
        };
        if age <= max_age_ms {
            continue;
        }
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let deleted = if is_dir {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if deleted.is_ok() {
            removed += 1;
        }
    }
    removed
}

/// A deleted entry, as the trash dialog lists it.
#[derive(Serialize, specta::Type)]
pub struct TrashItem {
    /// The item's directory name under `.margin/trash` — the handle the UI
    /// passes back to restore or purge it.
    pub id: String,
    pub name: String,
    /// Vault-relative path the entry had before it was deleted.
    pub path: String,
    pub is_dir: bool,
    /// Unix milliseconds (f64 because specta cannot export u64).
    pub deleted_at: f64,
    pub has_history: bool,
}

/// The vault-relative path recorded when the entry was trashed. Items written
/// by an earlier build have no sidecar; for those the single-child chain inside
/// `files/` is followed, which is exact for a file and the best available guess
/// for a folder (an ancestor directory is indistinguishable from the entry).
fn item_rel(item: &Path) -> Result<PathBuf, String> {
    if let Ok(recorded) = fs::read_to_string(item.join(REL_FILE)) {
        let rel = recorded.trim();
        if !rel.is_empty() {
            return Ok(PathBuf::from(rel));
        }
    }
    let mut dir = item.join(FILES_DIR);
    let mut rel = PathBuf::new();
    for _ in 0..MAX_WALK_DEPTH {
        let mut entries = fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read trash item: {e}"))?
            .flatten();
        let Some(first) = entries.next() else {
            return Err("Trash item holds nothing".into());
        };
        if entries.next().is_some() {
            break;
        }
        rel.push(first.file_name());
        let next = dir.join(rel.file_name().unwrap_or_default());
        if !next.is_dir() {
            return Ok(rel);
        }
        dir = next;
    }
    if rel.as_os_str().is_empty() {
        Err("Trash item has no recorded path".into())
    } else {
        Ok(rel)
    }
}

/// The item directory for `id`, refusing anything that is not one trash item's
/// own name (so `../` cannot address the vault).
fn item_dir(root: &str, id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.contains('/')
        || id.contains('\\')
        || id.contains("..")
        || !id_timestamp_millis(id).is_some_and(|ms| ms > 0)
    {
        return Err("Invalid trash id".into());
    }
    let item = Path::new(root).join(TRASH_ROOT).join(id);
    if !item.is_dir() {
        return Err("Trash item no longer exists".into());
    }
    Ok(item)
}

/// A path the entry can be restored to without replacing anything: the original
/// when free, otherwise `<name> (restored).<ext>`, `<name> (restored 2).<ext>`, …
///
/// Existence is compared case-folded because macOS and Windows would treat
/// `Note.md` and `note.md` as the same entry, and the restore must not depend on
/// the platform's answer.
pub(crate) fn unique_restore_path(root: &Path, rel: &Path) -> PathBuf {
    let Some(name) = rel.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return rel.to_path_buf();
    };
    let parent = root.join(rel.parent().unwrap_or(Path::new("")));
    let Ok(entries) = fs::read_dir(&parent) else {
        return rel.to_path_buf();
    };
    let taken: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_lowercase())
        .collect();
    if !taken.contains(&name.to_lowercase()) {
        return rel.to_path_buf();
    }

    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name.as_str(), ""),
    };
    for n in 1..1000 {
        let suffix = if n == 1 {
            " (restored)".to_string()
        } else {
            format!(" (restored {n})")
        };
        let candidate = format!("{stem}{suffix}{ext}");
        if !taken.contains(&candidate.to_lowercase()) {
            let mut restored = rel.to_path_buf();
            restored.set_file_name(candidate);
            return restored;
        }
    }
    rel.to_path_buf()
}

/// Remove the directories an item left behind once its payload moved out.
fn remove_empty_tree(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    let mut empty = true;
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir || !remove_empty_tree(&entry.path()) {
            empty = false;
        }
    }
    empty && fs::remove_dir(dir).is_ok()
}

/// The relative path as recorded in the sidecar: vault-relative, `/`-separated.
fn normalise_rel(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Every item currently in the trash, newest deletion first.
pub(crate) fn list_items(root: &str) -> Vec<TrashItem> {
    let Ok(entries) = fs::read_dir(Path::new(root).join(TRASH_ROOT)) else {
        return vec![];
    };
    let mut items: Vec<TrashItem> = entries
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            let deleted_at = id_timestamp_millis(&id)?;
            let item = entry.path();
            let rel = item_rel(&item).ok()?;
            Some(TrashItem {
                name: rel
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: normalise_rel(&rel),
                is_dir: item.join(FILES_DIR).join(&rel).is_dir(),
                has_history: item.join(HISTORY_DIR).join(&rel).exists(),
                deleted_at: deleted_at as f64,
                id,
            })
        })
        .collect();
    items.sort_by(|a, b| b.deleted_at.total_cmp(&a.deleted_at));
    items
}

/// Put a trashed entry back, returning the vault-relative path it landed on —
/// the original unless something now occupies it.
pub(crate) fn restore(root: &str, id: &str) -> Result<String, String> {
    let root_path = Path::new(root);
    let item = item_dir(root, id)?;
    let rel = item_rel(&item)?;

    let files_src = item.join(FILES_DIR).join(&rel);
    if !files_src.exists() {
        return Err("Trash item is missing its contents".into());
    }
    let dest_rel = unique_restore_path(root_path, &rel);
    let dest = root_path.join(&dest_rel);
    move_entry(&files_src, &dest)?;

    let history_src = item.join(HISTORY_DIR).join(&rel);
    if history_src.exists() {
        crate::history::merge_history_dirs(
            &history_src,
            &root_path.join(HISTORY_ROOT).join(&dest_rel),
        )?;
    }

    crate::index::tree::invalidate();
    if !dest.is_dir() {
        crate::index::upsert_path(root, &dest);
    }

    let _ = fs::remove_file(item.join(REL_FILE));
    remove_empty_tree(&item);

    Ok(normalise_rel(&dest_rel))
}

/// Remove one trashed item for good.
pub(crate) fn delete(root: &str, id: &str) -> Result<(), String> {
    let item = item_dir(root, id)?;
    fs::remove_dir_all(&item).map_err(|e| format!("Failed to delete trash item: {e}"))
}

/// Remove every trashed item. Returns the count as u32 so specta can export it.
pub(crate) fn empty(root: &str) -> u32 {
    let Ok(entries) = fs::read_dir(Path::new(root).join(TRASH_ROOT)) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let deleted = if is_dir {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if deleted.is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The single trash item under `vault`, as `delete_entry` would have made it.
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
        // The id carries the deletion time, which is what the purge ages out.
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

        // A different note now sits at the case-folded same name.
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
        // A fresh copy of the same name has been created since, with its own history.
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
}
