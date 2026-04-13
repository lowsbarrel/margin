use super::{FsEntry, TreeEntry, path_to_string};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Recursively walk an entire directory tree in a single call, returning all
/// entries (files and directories). Hidden entries (starting with `.`) are
/// skipped unless `include_hidden` is true.
#[tauri::command]
pub fn walk_directory(root: &str, include_hidden: bool) -> Result<Vec<FsEntry>, String> {
    let mut entries = Vec::new();
    walk_dir_impl(Path::new(root), include_hidden, &mut entries);
    Ok(entries)
}

fn walk_dir_impl(dir: &Path, include_hidden: bool, result: &mut Vec<FsEntry>) {
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in read.flatten() {
        let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
        if !include_hidden && name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path_str = path_to_string(entry.path());
        if is_dir {
            let child_path = Path::new(&path_str).to_path_buf();
            result.push(FsEntry {
                name,
                is_dir: true,
                path: path_str,
                modified,
            });
            walk_dir_impl(&child_path, include_hidden, result);
        } else {
            result.push(FsEntry {
                name,
                is_dir: false,
                path: path_str,
                modified,
            });
        }
    }
}

/// Build a flat, sorted, depth-annotated list of every currently-visible
/// tree row in a single native call.
#[tauri::command]
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

    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut entries: Vec<Raw> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
            if name.starts_with('.') {
                return None;
            }
            let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let modified = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            Some(Raw {
                name,
                path: e.path(),
                is_dir,
                modified,
            })
        })
        .collect();

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
pub fn build_subtree(
    folder: &str,
    depth_offset: usize,
    expanded: Vec<String>,
    sort_by: &str,
) -> Result<Vec<TreeEntry>, String> {
    let expanded_set: HashSet<String> = expanded.into_iter().collect();
    let mut results = Vec::new();
    build_tree_impl(Path::new(folder), depth_offset, &expanded_set, sort_by, &mut results);
    Ok(results)
}
