use super::{FsEntry, TreeEntry, path_to_string};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// One entry yielded by [`walk_dir`]. Carries the per-entry data every walker
/// in this crate needs, computed once from the `DirEntry`.
pub(crate) struct WalkItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// The entry itself is a symlink (never followed; `is_dir` is false for a
    /// symlink even when its target is a directory).
    pub is_symlink: bool,
    /// Seconds since UNIX epoch (modification time). 0 if unavailable.
    pub modified: u64,
}

/// Tells [`walk_dir`] what to do after visiting a directory entry.
pub(crate) enum WalkAction {
    /// Recurse into this directory.
    Recurse,
    /// Do not recurse into this directory.
    Skip,
}

/// Single shared recursive directory walker. Reads `dir`, and for each entry
/// (hidden filtering is left to the visitor) computes a [`WalkItem`] and
/// hands it to `visit`. The visitor returns a [`WalkAction`] controlling whether
/// the walker descends into directory entries.
///
/// Symlink-cycle protection: entries that are themselves symlinks are never
/// recursed into (the visitor still sees them), so a symlink loop inside the
/// vault cannot cause infinite recursion / stack overflow. This replaces the
/// previously hand-rolled `read_dir` recursion duplicated across the fs module
/// (it intentionally stays zero-dependency — no walkdir/ignore/jwalk).
pub(crate) fn walk_dir<F>(dir: &Path, visit: &mut F)
where
    F: FnMut(&WalkItem) -> WalkAction,
{
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in read.flatten() {
        let name = entry
            .file_name()
            .into_string()
            .unwrap_or_else(|s| s.to_string_lossy().into_owned());
        let file_type = entry.file_type().ok();
        let is_symlink = file_type.map(|ft| ft.is_symlink()).unwrap_or(false);
        let is_dir = file_type.map(|ft| ft.is_dir()).unwrap_or(false);
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let item = WalkItem {
            name,
            path: entry.path(),
            is_dir,
            is_symlink,
            modified,
        };
        let action = visit(&item);
        // Never follow symlinks — prevents infinite recursion on cyclic links.
        if is_dir && !is_symlink && matches!(action, WalkAction::Recurse) {
            walk_dir(&item.path, visit);
        }
    }
}

/// Depth-bounded recursive walk built on [`walk_dir`]. Descent is driven here
/// rather than by `walk_dir`'s own recursion so `max_depth` and the symlink
/// guard both apply; the visitor still chooses whether to descend by returning
/// [`WalkAction::Recurse`]. Depth counts directory levels — 0 descends nowhere.
/// Callers bound vault walks with `fs::MAX_WALK_DEPTH`.
pub(crate) fn walk_dir_capped<F>(dir: &Path, depth: usize, max_depth: usize, visit: &mut F)
where
    F: FnMut(&WalkItem) -> WalkAction,
{
    if depth >= max_depth {
        return;
    }
    let mut children: Vec<PathBuf> = Vec::new();
    walk_dir(dir, &mut |item| {
        let action = visit(item);
        if item.is_dir && !item.is_symlink && matches!(action, WalkAction::Recurse) {
            children.push(item.path.clone());
        }
        WalkAction::Skip
    });
    for child in children {
        walk_dir_capped(&child, depth + 1, max_depth, visit);
    }
}

/// Recursively walk an entire directory tree in a single call, returning all
/// entries (files and directories). Hidden entries (starting with `.`) are
/// skipped unless `include_hidden` is true.
#[tauri::command]
#[specta::specta]
pub fn walk_directory(root: &str, include_hidden: bool) -> Result<Vec<FsEntry>, String> {
    let mut entries = Vec::new();
    walk_dir(Path::new(root), &mut |item| {
        if !include_hidden && item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        entries.push(FsEntry {
            name: item.name.clone(),
            is_dir: item.is_dir,
            path: path_to_string(item.path.clone()),
            modified: item.modified,
        });
        WalkAction::Recurse
    });
    Ok(entries)
}

/// Build a flat, sorted, depth-annotated list of every currently-visible
/// tree row in a single native call.
///
/// `hidden` holds absolute paths the tree must not render. Hiding lives here —
/// in the rows the sidebar draws — and nowhere else: the walker, the watcher,
/// sync, export and the filename index all still see the folder.
#[tauri::command]
#[specta::specta]
pub fn build_visible_tree(
    root: &str,
    expanded: Vec<String>,
    sort_by: &str,
    hidden: Vec<String>,
) -> Result<Vec<TreeEntry>, String> {
    let expanded_set: HashSet<String> = expanded.into_iter().collect();
    let hidden_set = hidden_keys(hidden);
    let mut results = Vec::new();
    build_tree_impl(
        Path::new(root),
        0,
        &expanded_set,
        sort_by,
        &hidden_set,
        &mut results,
    );
    Ok(results)
}

/// Normalise the hidden paths once, so the per-entry test is a plain lookup.
fn hidden_keys(hidden: Vec<String>) -> HashSet<String> {
    hidden
        .into_iter()
        .map(|path| {
            crate::fs::normalise_slashes(&path)
                .trim_end_matches('/')
                .to_string()
        })
        .collect()
}

fn build_tree_impl(
    dir: &Path,
    depth: usize,
    expanded: &HashSet<String>,
    sort_by: &str,
    hidden: &HashSet<String>,
    result: &mut Vec<TreeEntry>,
) {
    struct Raw {
        name: String,
        path: std::path::PathBuf,
        is_dir: bool,
        modified: u64,
    }

    // Collect the immediate children of `dir` via the shared walker (with the
    // visitor always returning `Skip`, so it never descends — recursion below
    // is driven by the `expanded` set instead). This shares the entry-extraction
    // boilerplate and the symlink filtering with every other walker.
    let mut entries: Vec<Raw> = Vec::new();
    walk_dir(dir, &mut |item| {
        if !item.name.starts_with('.') {
            entries.push(Raw {
                name: item.name.clone(),
                path: item.path.clone(),
                is_dir: item.is_dir,
                modified: item.modified,
            });
        }
        WalkAction::Skip
    });

    // Sort: directories always before files, then within each group by sort_by.
    if sort_by == "date" {
        entries.sort_unstable_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            (false, false) => b.modified.cmp(&a.modified),
        });
    } else {
        entries.sort_unstable_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
    }

    for raw in entries {
        let path_str = path_to_string(raw.path.clone());
        if hidden.contains(path_str.as_str()) {
            continue;
        }
        let is_dir = raw.is_dir;
        result.push(TreeEntry {
            name: raw.name,
            path: path_str.clone(),
            is_dir,
            modified: raw.modified,
            depth,
        });
        if is_dir && expanded.contains(&path_str) {
            build_tree_impl(
                Path::new(&path_str),
                depth + 1,
                expanded,
                sort_by,
                hidden,
                result,
            );
        }
    }
}

/// Build the subtree for a single folder at a given depth offset.
/// Used for incremental expand — avoids rebuilding the entire tree.
#[tauri::command]
#[specta::specta]
pub fn build_subtree(
    folder: &str,
    // u32 (not usize) so specta can export it; tree depth never approaches u32::MAX.
    depth_offset: u32,
    expanded: Vec<String>,
    sort_by: &str,
    hidden: Vec<String>,
) -> Result<Vec<TreeEntry>, String> {
    let expanded_set: HashSet<String> = expanded.into_iter().collect();
    let hidden_set = hidden_keys(hidden);
    let mut results = Vec::new();
    build_tree_impl(
        Path::new(folder),
        depth_offset as usize,
        &expanded_set,
        sort_by,
        &hidden_set,
        &mut results,
    );
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "margin-walk-test-{}-{tag}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tree(root: &Path, hidden: Vec<String>) -> Vec<TreeEntry> {
        build_visible_tree(
            root.to_str().unwrap(),
            vec![root.join("attachments").to_string_lossy().into_owned()],
            "name",
            hidden,
        )
        .unwrap()
    }

    #[test]
    fn a_hidden_folder_is_omitted_from_the_tree_but_stays_on_disk() {
        let root = temp_dir("hidden");
        fs::create_dir_all(root.join("attachments")).unwrap();
        fs::create_dir(root.join("notes")).unwrap();
        fs::write(root.join("attachments/pic.png"), b"x").unwrap();

        let hidden = root.join("attachments").to_string_lossy().into_owned();
        let rows = tree(&root, vec![hidden]);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();

        assert_eq!(names, vec!["notes"]);
        assert!(
            root.join("attachments/pic.png").exists(),
            "hiding is a presentation rule — the file must still be there"
        );
        assert_eq!(
            super::super::walk_directory(root.to_str().unwrap(), false)
                .unwrap()
                .iter()
                .filter(|e| e.name == "attachments")
                .count(),
            1,
            "every walker must still see the folder"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn nothing_is_hidden_without_a_hidden_entry() {
        let root = temp_dir("shown");
        fs::create_dir(root.join("attachments")).unwrap();

        let rows = tree(&root, Vec::new());

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "attachments");

        fs::remove_dir_all(&root).ok();
    }
}
