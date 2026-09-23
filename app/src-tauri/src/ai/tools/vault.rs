use super::{
    MAX_GREP_FILE_BYTES, MAX_GREP_MATCHES, MAX_NOTE_BYTES, MAX_NOTE_LINES, MAX_RECENT_NOTES,
    MAX_TAG_NOTES, SEARCH_LIMIT, note_label, optional_int, rel_path, required_str, resolve,
};
use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn search(root: &str, args: &Value) -> Result<String, String> {
    let query = required_str(args, "query")?;
    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("all");
    let expression = crate::index::build_match_for(query, mode);
    if expression.is_empty() {
        return Err("The query had no searchable words.".to_string());
    }
    let hits = crate::index::search_match(root, &expression, SEARCH_LIMIT)?;
    let mut out = String::new();
    for hit in hits {
        let snippet = hit.snippet.split_whitespace().collect::<Vec<_>>().join(" ");
        out.push_str(&format!(
            "[[{}]] — {}\n  {snippet}\n",
            note_label(&hit.name),
            rel_path(root, &hit.path)
        ));
    }
    Ok(out)
}

pub(super) fn grep(root: &str, args: &Value) -> Result<String, String> {
    let pattern = required_str(args, "pattern")?;
    let regex = Regex::new(pattern).map_err(|e| format!("Invalid regular expression: {e}"))?;
    let from = match args.get("folder").and_then(|v| v.as_str()) {
        Some(folder) if !folder.trim().is_empty() => {
            let resolved = resolve(root, folder.trim())?;
            if !resolved.is_dir() {
                return Err(format!("Not a folder: {}", folder.trim()));
            }
            resolved
        }
        _ => PathBuf::from(root),
    };

    let mut files = Vec::new();
    crate::fs::collect_md_paths(&from, &mut files);
    files.sort();

    let mut out = String::new();
    let mut matches = 0usize;
    for file in files {
        if matches >= MAX_GREP_MATCHES {
            break;
        }
        let Ok(meta) = std::fs::metadata(&file) else {
            continue;
        };
        if meta.len() > MAX_GREP_FILE_BYTES {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&file) else {
            continue;
        };
        let shown = rel_path(root, &file.to_string_lossy());
        for (index, line) in body.lines().enumerate() {
            if matches >= MAX_GREP_MATCHES {
                break;
            }
            if !regex.is_match(line) {
                continue;
            }
            matches += 1;
            let trimmed = line.trim();
            out.push_str(&format!("{}:{}: {}\n", shown, index + 1, trimmed));
        }
    }
    Ok(out)
}

pub(super) fn find_notes(root: &str, args: &Value) -> Result<String, String> {
    let name = required_str(args, "name")?;
    let entries = crate::index::tree::search(root, name, SEARCH_LIMIT as usize);
    let mut out = String::new();
    for entry in entries.into_iter().filter(|e| !e.is_dir) {
        out.push_str(&format!(
            "[[{}]] — {}\n",
            note_label(&entry.name),
            rel_path(root, &entry.path)
        ));
    }
    Ok(out)
}

pub(super) fn read_note(root: &str, args: &Value) -> Result<String, String> {
    let raw = required_str(args, "path")?;
    let path = resolve(root, raw)?;
    let body = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read {raw}: {e}"))?;
    let lines: Vec<&str> = body.lines().collect();
    let total = lines.len();

    let from = optional_int(args, "from_line").unwrap_or(1).max(1) as usize;
    let requested_to = optional_int(args, "to_line")
        .map(|v| v.max(0) as usize)
        .unwrap_or(total)
        .min(total);
    if from > total {
        return Err(format!("{raw} has only {total} lines"));
    }
    let to = requested_to.min(from + MAX_NOTE_LINES - 1).max(from);

    let mut out = format!(
        "{} (lines {from}-{to} of {total})\n",
        rel_path(root, &path.to_string_lossy())
    );
    for line in &lines[from - 1..to] {
        out.push_str(line);
        out.push('\n');
        if out.len() >= MAX_NOTE_BYTES {
            out.push_str("… (truncated)\n");
            break;
        }
    }
    Ok(out)
}

pub(super) fn list_tags(root: &str) -> Result<String, String> {
    let tags = crate::index::index_tags(root)?;
    let mut out = String::new();
    for tag in tags {
        out.push_str(&format!("#{} ({} notes)\n", tag.tag, tag.count));
    }
    Ok(out)
}

pub(super) fn notes_with_tag(root: &str, args: &Value) -> Result<String, String> {
    let wanted = required_str(args, "tag")?
        .trim_start_matches('#')
        .to_lowercase();
    let tags = crate::index::index_tags(root)?;
    let Some(tag) = tags.into_iter().find(|t| t.tag.to_lowercase() == wanted) else {
        return Ok(String::new());
    };
    let mut out = String::new();
    for path in tag.files.into_iter().take(MAX_TAG_NOTES) {
        out.push_str(&format!(
            "[[{}]] — {}\n",
            note_label(&path),
            rel_path(root, &path)
        ));
    }
    Ok(out)
}

pub(super) fn backlinks(root: &str, args: &Value) -> Result<String, String> {
    let raw = required_str(args, "path")?;
    let path = resolve(root, raw)?;
    let links = crate::index::index_backlinks(root, &path.to_string_lossy())?;
    let mut out = String::new();
    for link in links {
        out.push_str(&format!(
            "[[{}]] — {}\n",
            note_label(&link.name),
            rel_path(root, &link.path)
        ));
    }
    Ok(out)
}

pub(super) fn recent_notes(root: &str, args: &Value) -> Result<String, String> {
    let days = optional_int(args, "days").unwrap_or(7).clamp(1, 3650);
    let limit = optional_int(args, "limit")
        .unwrap_or(10)
        .clamp(1, MAX_RECENT_NOTES as i64) as usize;
    let cutoff = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        - days * 86_400;

    let mut files = Vec::new();
    crate::fs::collect_md_paths(Path::new(root), &mut files);
    let mut recent: Vec<(i64, String)> = Vec::new();
    for file in files {
        let Ok(meta) = std::fs::metadata(&file) else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let Ok(stamp) = modified.duration_since(UNIX_EPOCH) else {
            continue;
        };
        let stamp = stamp.as_secs() as i64;
        if stamp < cutoff {
            continue;
        }
        recent.push((stamp, file.to_string_lossy().to_string()));
    }
    recent.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    recent.truncate(limit);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut out = String::new();
    for (stamp, path) in recent {
        let age_days = (now - stamp).max(0) / 86_400;
        let age = match age_days {
            0 => "today".to_string(),
            1 => "yesterday".to_string(),
            n => format!("{n} days ago"),
        };
        out.push_str(&format!(
            "[[{}]] — {} ({age})\n",
            note_label(&path),
            rel_path(root, &path)
        ));
    }
    Ok(out)
}
