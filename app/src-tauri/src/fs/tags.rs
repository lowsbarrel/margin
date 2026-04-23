use super::{path_to_string, search::collect_md_paths};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct TagInfo {
    pub tag: String,
    pub count: usize,
    pub files: Vec<String>,
}

/// Extract `#tag` tokens from markdown content.
fn extract_tags_from_content(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        let bytes = trimmed.as_bytes();

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
