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
}
