use super::HASH_CHARS;
use crate::fs::{WalkAction, normalise_slashes, path_to_string, trash, walk_dir};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

pub(crate) const SWEEP_GRACE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

fn referenced_names(md: &str, out: &mut HashSet<String>) {
    let mut pos = 0;
    while let Some(found) = md[pos..].find("![[") {
        let start = pos + found + 3;
        match md[start..].find("]]") {
            Some(close) => {
                target(&md[start..start + close], out);
                pos = start + close + 2;
            }
            None => break,
        }
    }

    let mut pos = 0;
    while let Some(found) = md[pos..].find("](") {
        let start = pos + found + 2;
        match md[start..].find(')') {
            Some(close) => {
                target(&md[start..start + close], out);
                pos = start + close + 1;
            }
            None => break,
        }
    }

    let mut pos = 0;
    while let Some(found) = md[pos..].find("src=") {
        let start = pos + found + 4;
        let quote = md[start..].chars().next().unwrap_or(' ');
        let (body_start, end) = if quote == '"' || quote == '\'' {
            (
                start + quote.len_utf8(),
                md[start + 1..].find(quote).map(|i| start + 1 + i),
            )
        } else {
            (
                start,
                md[start..]
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .map(|i| start + i),
            )
        };
        match end {
            Some(end) => {
                target(&md[body_start..end], out);
                pos = end;
            }
            None => break,
        }
    }
}

fn target(raw: &str, out: &mut HashSet<String>) {
    let raw = raw.trim().trim_start_matches('<').trim_end_matches('>');
    let raw = raw.split_whitespace().next().unwrap_or("");
    let raw = raw.split('|').next().unwrap_or(raw);
    if raw.is_empty() {
        return;
    }
    let decoded = crate::ipc::percent_decode_lossy(raw);
    let decoded = normalise_slashes(&decoded);
    if let Some(name) = decoded.rsplit('/').next()
        && !name.is_empty()
    {
        out.insert(name.to_string());
    }
}

pub(crate) fn unused_files(root: &str, folder: &str) -> Result<Vec<String>, String> {
    let dir = Path::new(root).join(folder);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut referenced: HashSet<String> = HashSet::new();
    let mut notes: Vec<std::path::PathBuf> = Vec::new();
    walk_dir(Path::new(root), &mut |item| {
        if item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        if item.is_dir {
            return WalkAction::Recurse;
        }
        if item
            .path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        {
            notes.push(item.path.clone());
        }
        WalkAction::Skip
    });

    for note in notes {
        if let Ok(md) = fs::read_to_string(&note) {
            referenced_names(&md, &mut referenced);
        }
    }

    let mut unused: Vec<String> = Vec::new();
    walk_dir(&dir, &mut |item| {
        if item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        if item.is_dir {
            return WalkAction::Recurse;
        }
        if referenced.contains(&item.name) {
            return WalkAction::Skip;
        }
        let rel = item
            .path
            .strip_prefix(&dir)
            .map(|rel| path_to_string(rel.to_path_buf()))
            .unwrap_or_else(|_| item.name.clone());
        unused.push(format!("{folder}/{rel}"));
        WalkAction::Skip
    });
    unused.sort();
    Ok(unused)
}

fn has_hash_tail(stem: &str) -> bool {
    stem.rsplit_once('-').is_some_and(|(head, tail)| {
        !head.is_empty()
            && tail.len() == HASH_CHARS
            && tail.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

pub(crate) fn is_stored_name(name: &str) -> bool {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    has_hash_tail(stem)
        || stem.rsplit_once('-').is_some_and(|(head, n)| {
            !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && has_hash_tail(head)
        })
}

pub(crate) fn sweep_unused(root: &str, folder: &str, grace: Duration) -> Result<u32, String> {
    let now = SystemTime::now();
    let mut swept = 0;
    for rel in unused_files(root, folder)? {
        let path = Path::new(root).join(&rel);
        let stored = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(is_stored_name);
        let stale = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= grace);
        if !stored || !stale {
            continue;
        }
        let Ok(canonical) = path.canonicalize() else {
            continue;
        };
        match trash::trash_entry(root, &canonical) {
            Ok(()) => swept += 1,
            Err(e) => eprintln!("Failed to trash unused attachment {rel}: {e}"),
        }
    }
    Ok(swept)
}
