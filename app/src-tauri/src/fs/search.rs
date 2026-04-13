use super::{FsEntry, path_to_string, atomic_write};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct ContentMatch {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub context: String,
}

#[derive(Serialize)]
pub struct TagInfo {
    pub tag: String,
    pub count: usize,
    pub files: Vec<String>,
}

#[tauri::command]
pub fn search_files(root: &str, query: &str) -> Result<Vec<FsEntry>, String> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    const MAX_RESULTS: usize = 200;
    const MAX_DEPTH: usize = 10;
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
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if results.len() >= max_results {
            return;
        }
        let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            collect_files_recursive(&path, query, results, depth + 1, max_depth, max_results);
        } else if name.to_lowercase().contains(query) {
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            results.push(FsEntry {
                name,
                is_dir: false,
                path: path_to_string(path),
                modified,
            });
        }
    }
}

#[tauri::command]
pub fn search_file_contents(
    root: &str,
    query: &str,
    case_sensitive: bool,
) -> Result<Vec<ContentMatch>, String> {
    if query.is_empty() {
        return Ok(vec![]);
    }
    const MAX_RESULTS: usize = 500;
    const MAX_DEPTH: usize = 10;
    const MAX_FILES: usize = 5000;

    // Phase 1: collect .md paths (lightweight, single-thread)
    let mut md_paths = Vec::new();
    collect_md_paths(Path::new(root), &mut md_paths, 0, MAX_DEPTH);
    md_paths.truncate(MAX_FILES);

    // Phase 2: pre-compute query once
    let query_cmp = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    // Phase 3: search in parallel, each file produces its own matches
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
                let content_lower = content.to_lowercase();
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

    // Phase 4: flatten & truncate
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
    ContentMatch {
        path: path.to_string(),
        name: name.to_string(),
        line: line_idx + 1,
        column: col + 1,
        context: line[ctx_start..ctx_end].to_string(),
    }
}

#[tauri::command]
pub fn replace_in_file(
    path: &str,
    search: &str,
    replace: &str,
    case_sensitive: bool,
) -> Result<usize, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {e}"))?;
    let (new_content, count) = if case_sensitive {
        let count = content.matches(search).count();
        (content.replace(search, replace), count)
    } else {
        let mut result = String::with_capacity(content.len());
        let search_lower = search.to_lowercase();
        let mut last_end = 0;
        let content_lower = content.to_lowercase();
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
    Ok(count)
}

// ─── Tag extraction ──────────────────────────────────────────────────────────

/// Extract `#tag` tokens from markdown content.
fn extract_tags_from_content(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        let bytes = trimmed.as_bytes();

        // Toggle code block state
        if bytes.starts_with(b"```") || bytes.starts_with(b"~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }

        // Skip markdown headings (# Heading, ## Heading, …)
        if bytes.first() == Some(&b'#') {
            let hash_end = bytes.iter().position(|&b| b != b'#').unwrap_or(bytes.len());
            if hash_end >= bytes.len() || bytes[hash_end] == b' ' {
                continue;
            }
        }

        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'#' {
                let prev_ok = i == 0 || {
                    let p = bytes[i - 1];
                    !p.is_ascii_alphanumeric() && p != b'_' && p != b'#'
                };
                if prev_ok && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
                    let start = i + 1;
                    let mut j = start;
                    while j < bytes.len()
                        && (bytes[j].is_ascii_alphanumeric()
                            || bytes[j] == b'-'
                            || bytes[j] == b'_'
                            || bytes[j] == b'/')
                    {
                        j += 1;
                    }
                    let tag = std::str::from_utf8(&bytes[start..j])
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if !tag.is_empty() {
                        tags.push(tag);
                    }
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
    }

    tags
}

/// Scan all `.md` files under `root` and collect unique `#tag` tokens.
#[tauri::command]
pub fn list_all_tags(root: &str) -> Result<Vec<TagInfo>, String> {
    let mut md_paths = Vec::new();
    collect_md_paths(Path::new(root), &mut md_paths, 0, 10);

    let per_file: Vec<(String, Vec<String>)> = md_paths
        .into_par_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let tags = extract_tags_from_content(&content);
            Some((path_to_string(path), tags))
        })
        .collect();

    let mut tag_files: HashMap<String, Vec<String>> = HashMap::new();
    for (path_str, tags) in per_file {
        let mut seen: HashSet<String> = HashSet::new();
        for tag in tags {
            if seen.insert(tag.clone()) {
                tag_files.entry(tag).or_default().push(path_str.clone());
            }
        }
    }

    let mut result: Vec<TagInfo> = tag_files
        .into_iter()
        .map(|(tag, files)| {
            let count = files.len();
            TagInfo { tag, count, files }
        })
        .collect();

    result.sort_by(|a, b| b.count.cmp(&a.count).then(a.tag.cmp(&b.tag)));
    Ok(result)
}

pub(crate) fn collect_md_paths(dir: &Path, out: &mut Vec<std::path::PathBuf>, depth: usize, max_depth: usize) {
    if depth >= max_depth {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            collect_md_paths(&path, out, depth + 1, max_depth);
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
}
