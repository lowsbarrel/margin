use super::{
    FsEntry, MAX_WALK_DEPTH, VaultPathState, WalkAction, atomic_write, ensure_in_vault,
    walk_dir_capped,
};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_RESULTS: usize = 200;

#[tauri::command(async)]
#[specta::specta]
pub fn search_files(root: &str, query: &str) -> Result<Vec<FsEntry>, String> {
    Ok(crate::index::tree::search(root, query, MAX_RESULTS))
}

#[tauri::command]
#[specta::specta]
pub fn replace_in_file(
    path: &str,
    search: &str,
    replace: &str,
    case_sensitive: bool,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<u32, String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    let content = fs::read_to_string(&p).map_err(|e| format!("Failed to read file: {e}"))?;
    let (new_content, count) = if case_sensitive {
        let count = content.matches(search).count();
        (content.replace(search, replace), count)
    } else {
        // ASCII-folding preserves byte length, so offsets found in the lowered copy still index the original.
        let mut result = String::with_capacity(content.len());
        let search_lower = search.to_ascii_lowercase();
        let mut last_end = 0;
        let content_lower = content.to_ascii_lowercase();
        let mut count = 0usize;
        while let Some(start) = content_lower[last_end..].find(&search_lower) {
            let abs_start = last_end + start;
            result.push_str(&content[last_end..abs_start]);
            result.push_str(replace);
            last_end = abs_start + search.len();
            count += 1;
        }
        result.push_str(&content[last_end..]);
        (result, count)
    };
    if count > 0 {
        atomic_write(&p, new_content.as_bytes())?;
    }
    Ok(count as u32)
}

pub(crate) fn collect_md_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    walk_dir_capped(dir, 0, MAX_WALK_DEPTH, &mut |item| {
        if item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        if item.is_dir {
            return WalkAction::Recurse;
        }
        if item.name.ends_with(".md") {
            out.push(item.path.clone());
        }
        WalkAction::Skip
    });
}
