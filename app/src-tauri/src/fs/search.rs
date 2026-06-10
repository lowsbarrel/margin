use super::{atomic_write, path_to_string, walk_dir, FsEntry, WalkAction};
use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize, specta::Type)]
pub struct ContentMatch {
    pub path: String,
    pub name: String,
    #[specta(type = u32)]
    pub line: usize,
    #[specta(type = u32)]
    pub column: usize,
    pub context: String,
}

/// Maximum directory nesting that file/content search and tag scanning will
/// recurse into. Centralized here (was duplicated as a literal `10` across
/// search.rs and tags.rs). Entries nested deeper are silently skipped — rare in
/// a notes vault, but kept as a guard against pathological trees / symlink loops
/// (the shared walker also refuses to follow symlinks).
pub(crate) const MAX_WALK_DEPTH: usize = 10;

#[tauri::command]
#[specta::specta]
pub fn search_files(root: &str, query: &str) -> Result<Vec<FsEntry>, String> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    const MAX_RESULTS: usize = 200;
    const MAX_DEPTH: usize = MAX_WALK_DEPTH;
    collect_files_recursive(
        Path::new(root),
        &query_lower,
        &mut results,
        0,
        MAX_DEPTH,
        MAX_RESULTS,
    );
    // Sort: exact prefix matches first, then alphabetical
    results.sort_by(|a, b| {
        let a_starts = a.name.to_lowercase().starts_with(&query_lower);
        let b_starts = b.name.to_lowercase().starts_with(&query_lower);
        match (a_starts, b_starts) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    Ok(results)
}

fn collect_files_recursive(
    dir: &Path,
    query: &str,
    results: &mut Vec<FsEntry>,
    depth: usize,
    max_depth: usize,
    max_results: usize,
) {
    if depth >= max_depth || results.len() >= max_results {
        return;
    }
    // Collect this level via the shared walker (it never descends — `Skip`); the
    // depth cap and `query` filtering stay here, matching the original behavior.
    let mut child_dirs: Vec<std::path::PathBuf> = Vec::new();
    walk_dir(dir, &mut |item| {
        if results.len() >= max_results || item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        if item.is_dir {
            child_dirs.push(item.path.clone());
        } else if item.name.to_lowercase().contains(query) {
            results.push(FsEntry {
                name: item.name.clone(),
                is_dir: false,
                path: path_to_string(item.path.clone()),
                modified: item.modified,
            });
        }
        WalkAction::Skip
    });
    for child in child_dirs {
        if results.len() >= max_results {
            return;
        }
        collect_files_recursive(&child, query, results, depth + 1, max_depth, max_results);
    }
}

#[tauri::command]
#[specta::specta]
pub fn search_file_contents(
    root: &str,
    query: &str,
    case_sensitive: bool,
) -> Result<Vec<ContentMatch>, String> {
    if query.is_empty() {
        return Ok(vec![]);
    }
    const MAX_RESULTS: usize = 500;
    const MAX_DEPTH: usize = MAX_WALK_DEPTH;
    const MAX_FILES: usize = 5000;

    // Collect .md paths
    let mut md_paths = Vec::new();
    collect_md_paths(Path::new(root), &mut md_paths, 0, MAX_DEPTH);
    md_paths.truncate(MAX_FILES);

    // Pre-compute query once. For the case-insensitive path we use
    // `to_ascii_lowercase` (length-preserving) rather than full Unicode
    // `to_lowercase`, because the match column computed on the lowered line is
    // used to slice the ORIGINAL line in `build_content_match`. A Unicode fold
    // can change byte length (e.g. 'İ' → "i\u{307}'), desyncing those offsets
    // and risking a panic on a non-char-boundary slice. The tradeoff is that
    // only ASCII letters are case-folded; non-ASCII case-insensitivity is not
    // supported on this path.
    let query_cmp = if case_sensitive {
        query.to_string()
    } else {
        query.to_ascii_lowercase()
    };

    // Search in parallel
    let all_matches: Vec<Vec<ContentMatch>> = md_paths
        .into_par_iter()
        .map(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => return vec![],
            };
            let path_str = path_to_string(path);
            let mut matches = Vec::new();

            if case_sensitive {
                for (line_idx, line) in content.lines().enumerate() {
                    let mut search_from = 0;
                    while let Some(col) = line[search_from..].find(&*query_cmp) {
                        let actual_col = search_from + col;
                        matches.push(build_content_match(
                            &path_str,
                            &name,
                            line,
                            line_idx,
                            actual_col,
                            query.len(),
                        ));
                        search_from = actual_col + query_cmp.len();
                    }
                }
            } else {
                // ASCII-fold (length-preserving) so byte offsets into the lowered
                // copy stay valid when slicing the original line below.
                let content_lower = content.to_ascii_lowercase();
                let lines_lower: Vec<&str> = content_lower.lines().collect();
                let lines_orig: Vec<&str> = content.lines().collect();
                for line_idx in 0..lines_lower.len() {
                    let line_l = lines_lower[line_idx];
                    let line_o = if line_idx < lines_orig.len() {
                        lines_orig[line_idx]
                    } else {
                        line_l
                    };
                    let mut search_from = 0;
                    while let Some(col) = line_l[search_from..].find(&*query_cmp) {
                        let actual_col = search_from + col;
                        matches.push(build_content_match(
                            &path_str,
                            &name,
                            line_o,
                            line_idx,
                            actual_col,
                            query.len(),
                        ));
                        search_from = actual_col + query_cmp.len();
                    }
                }
            }
            matches
        })
        .collect();

    // Flatten & truncate
    let mut results: Vec<ContentMatch> = all_matches.into_iter().flatten().collect();
    results.truncate(MAX_RESULTS);
    Ok(results)
}

/// Build a ContentMatch with surrounding context, snapping to char boundaries.
fn build_content_match(
    path: &str,
    name: &str,
    line: &str,
    line_idx: usize,
    col: usize,
    query_len: usize,
) -> ContentMatch {
    let mut ctx_start = col.saturating_sub(30);
    while ctx_start > 0 && !line.is_char_boundary(ctx_start) {
        ctx_start -= 1;
    }
    let mut ctx_end = std::cmp::min(line.len(), col + query_len + 60);
    while ctx_end < line.len() && !line.is_char_boundary(ctx_end) {
        ctx_end += 1;
    }
    // `col` is a byte offset into `line`; convert to a 1-based CHARACTER offset
    // so the JS editor's "jump to match" lands correctly on lines containing
    // multibyte UTF-8 (matches the char-offset convention in text/wiki_links.rs).
    let char_col = line[..col].chars().count() + 1;
    ContentMatch {
        path: path.to_string(),
        name: name.to_string(),
        line: line_idx + 1,
        column: char_col,
        context: line[ctx_start..ctx_end].to_string(),
    }
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
