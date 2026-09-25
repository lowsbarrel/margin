use super::{FsEntry, TreeEntry, path_to_string};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const MAX_WALK_DEPTH: usize = 10;

pub(crate) struct WalkItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub modified: u64,
}

pub(crate) enum WalkAction {
    Recurse,
    Skip,
}

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
        // Symlinks are never followed: a cycle of links inside the vault would recurse without bound.
        if is_dir && !is_symlink && matches!(action, WalkAction::Recurse) {
            walk_dir(&item.path, visit);
        }
    }
}

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

#[tauri::command(async)]
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

#[tauri::command(async)]
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

#[tauri::command(async)]
#[specta::specta]
pub fn build_subtree(
    folder: &str,
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
