use super::{path_to_string, FsEntry, TreeEntry};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// One entry yielded by [`walk_dir`]. Carries the per-entry data every walker
/// in this crate needs, computed once from the `DirEntry`.
pub(crate) struct WalkItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
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

/// Single shared recursive directory walker. Reads `dir`, and for each non-…
/// (hidden filtering is left to the visitor) entry computes a [`WalkItem`] and
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
            modified,
        };
        let action = visit(&item);
        // Never follow symlinks — prevents infinite recursion on cyclic links.
        if is_dir && !is_symlink && matches!(action, WalkAction::Recurse) {
            walk_dir(&item.path, visit);
        }
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
#[tauri::command]
#[specta::specta]
pub fn build_visible_tree(
    root: &str,
    expanded: Vec<String>,
    sort_by: &str,
) -> Result<Vec<TreeEntry>, String> {
    let expanded_set: HashSet<String> = expanded.into_iter().collect();
    let mut results = Vec::new();
    build_tree_impl(Path::new(root), 0, &expanded_set, sort_by, &mut results);
    Ok(results)
}

fn build_tree_impl(
    dir: &Path,
    depth: usize,
    expanded: &HashSet<String>,
    sort_by: &str,
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
        let is_dir = raw.is_dir;
        result.push(TreeEntry {
            name: raw.name,
            path: path_str.clone(),
            is_dir,
            modified: raw.modified,
            depth,
        });
        if is_dir && expanded.contains(&path_str) {
            build_tree_impl(Path::new(&path_str), depth + 1, expanded, sort_by, result);
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
) -> Result<Vec<TreeEntry>, String> {
    let expanded_set: HashSet<String> = expanded.into_iter().collect();
    let mut results = Vec::new();
    build_tree_impl(
        Path::new(folder),
        depth_offset as usize,
        &expanded_set,
        sort_by,
        &mut results,
    );
    Ok(results)
}
