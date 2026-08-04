use super::{atomic_write, walk_dir, FsEntry, WalkAction};
use std::fs;
use std::path::Path;

/// Maximum directory nesting that file/content search and tag scanning will
/// recurse into. Centralized here (was duplicated as a literal `10` across
/// search.rs and tags.rs). Entries nested deeper are silently skipped — rare in
/// a notes vault, but kept as a guard against pathological trees / symlink loops
/// (the shared walker also refuses to follow symlinks).
pub(crate) const MAX_WALK_DEPTH: usize = 10;

/// Filename search results returned per query. The old disk-walking
/// implementation capped at 200 purely to bound the walk; the cap now only
/// bounds how much is sent over IPC, since ranking happens in memory.
const MAX_RESULTS: usize = 200;

/// Ranked filename search.
///
/// This used to re-walk the whole vault from disk on every keystroke. It is now
/// served from the in-memory vault tree in [`crate::index::tree`], which walks
/// once and answers subsequent queries out of a prefix trie. Results come back
/// ranked (name prefix → token prefix → substring → path → fuzzy) rather than
/// merely alphabetical, so the caller can render them directly.
#[tauri::command]
#[specta::specta]
pub fn search_files(root: &str, query: &str) -> Result<Vec<FsEntry>, String> {
    Ok(crate::index::tree::search(root, query, MAX_RESULTS))
}

// Returns the number of replacements as u32 (not usize) so specta can export it;
// the count is bounded by the file's match count and never approaches u32::MAX.
#[tauri::command]
#[specta::specta]
pub fn replace_in_file(
    path: &str,
    search: &str,
    replace: &str,
    case_sensitive: bool,
) -> Result<u32, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {e}"))?;
    let (new_content, count) = if case_sensitive {
        let count = content.matches(search).count();
        (content.replace(search, replace), count)
    } else {
        // ASCII-fold (length-preserving) both sides: the match offsets found in
        // `content_lower` are used to slice the ORIGINAL `content`, so the lowered
        // copy must have identical byte lengths. Full Unicode `to_lowercase` could
        // change length and corrupt the output or panic on a non-char-boundary
        // slice. Limitation: only ASCII letters are matched case-insensitively.
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
        atomic_write(Path::new(path), new_content.as_bytes())?;
    }
    Ok(count as u32)
}

pub(crate) fn collect_md_paths(
    dir: &Path,
    out: &mut Vec<std::path::PathBuf>,
    depth: usize,
    max_depth: usize,
) {
    if depth >= max_depth {
        return;
    }
    // One level via the shared walker; recursion (and the depth cap) stays here.
    let mut child_dirs: Vec<std::path::PathBuf> = Vec::new();
    walk_dir(dir, &mut |item| {
        if item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        if item.is_dir {
            child_dirs.push(item.path.clone());
        } else if item.name.ends_with(".md") {
            out.push(item.path.clone());
        }
        WalkAction::Skip
    });
    for child in child_dirs {
        collect_md_paths(&child, out, depth + 1, max_depth);
    }
}
