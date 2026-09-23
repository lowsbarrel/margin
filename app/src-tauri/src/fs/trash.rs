use super::MAX_WALK_DEPTH;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) mod commands;
#[cfg(test)]
mod tests;

pub(crate) const PURGE_AGE_MS: u64 = 30 * 24 * 60 * 60 * 1000;

const TRASH_ROOT: &str = ".margin/trash";
const HISTORY_ROOT: &str = ".margin/history";
const FILES_DIR: &str = "files";
const HISTORY_DIR: &str = "history";
const REL_FILE: &str = "path";

static TRASH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn id_timestamp_millis(id: &str) -> Option<u64> {
    id.split('-').next()?.parse().ok()
}

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

// ensure_in_vault canonicalizes (\\?\C:\ on Windows), so the path is stripped from the canonical root and re-joined onto the raw one.
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

pub(crate) fn trash_entry(root: &str, path: &Path) -> Result<(), String> {
    let root_path = Path::new(root);
    let rel = vault_relative(root, path)?;
    let item = new_trash_item(&root_path.join(TRASH_ROOT));

    move_entry(path, &item.join(FILES_DIR).join(&rel))?;

    if let Err(e) = fs::write(item.join(REL_FILE), normalise_rel(&rel)) {
        eprintln!("Failed to record trash item path: {e}");
    }

    let history_src = root_path.join(HISTORY_ROOT).join(&rel);
    if history_src.exists()
        && let Err(e) = move_entry(&history_src, &item.join(HISTORY_DIR).join(&rel))
    {
        eprintln!("Failed to move history into trash: {e}");
    }
    Ok(())
}

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

#[derive(Serialize, specta::Type)]
pub struct TrashItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub deleted_at: f64,
    pub has_history: bool,
}

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

fn normalise_rel(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

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

pub(crate) fn delete(root: &str, id: &str) -> Result<(), String> {
    let item = item_dir(root, id)?;
    fs::remove_dir_all(&item).map_err(|e| format!("Failed to delete trash item: {e}"))
}

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
