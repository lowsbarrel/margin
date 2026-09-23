mod vault;

use crate::fs::ensure_within;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const MAX_OUTPUT_CHARS: usize = 8000;
const SEARCH_LIMIT: u32 = 20;
const MAX_NOTE_LINES: usize = 400;
const MAX_NOTE_BYTES: usize = 40 * 1024;
const MAX_GREP_FILE_BYTES: u64 = 1024 * 1024;
const MAX_GREP_MATCHES: usize = 200;
const MAX_TAG_NOTES: usize = 50;
const MAX_RECENT_NOTES: usize = 50;

pub fn specs() -> Vec<super::chat::ToolSpec> {
    vec![
        super::chat::ToolSpec {
            name: "search",
            description: "Full-text search across every note in the vault. Use this first for \
                          most questions. mode 'all' requires every word (default), 'any' \
                          requires at least one, 'phrase' matches the exact phrase.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words to look for." },
                    "mode": { "type": "string", "enum": ["all", "any", "phrase"] }
                },
                "required": ["query"]
            }),
        },
        super::chat::ToolSpec {
            name: "grep",
            description: "Regular-expression search inside note contents, line by line. Use it \
                          for exact wording, identifiers or patterns that full-text search would \
                          tokenize away.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "A regular expression." },
                    "folder": {
                        "type": "string",
                        "description": "Vault-relative folder to restrict the search to."
                    }
                },
                "required": ["pattern"]
            }),
        },
        super::chat::ToolSpec {
            name: "find_notes",
            description: "Find notes by file name when you know what a note is called.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Part of the note name." }
                },
                "required": ["name"]
            }),
        },
        super::chat::ToolSpec {
            name: "read_note",
            description: "Read a note, optionally a line range. Paths come from a previous tool \
                          result; they may be vault-relative or absolute.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "from_line": { "type": "integer", "description": "1-based, inclusive." },
                    "to_line": { "type": "integer", "description": "1-based, inclusive." }
                },
                "required": ["path"]
            }),
        },
        super::chat::ToolSpec {
            name: "list_tags",
            description: "Every #tag in the vault with how many notes carry it.",
            parameters: json!({ "type": "object", "properties": {} }),
        },
        super::chat::ToolSpec {
            name: "notes_with_tag",
            description: "The notes carrying one #tag.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "tag": { "type": "string", "description": "Tag name, with or without '#'." }
                },
                "required": ["tag"]
            }),
        },
        super::chat::ToolSpec {
            name: "backlinks",
            description: "The notes that link to a given note.",
            parameters: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        },
        super::chat::ToolSpec {
            name: "recent_notes",
            description: "Notes modified in the last N days, most recent first. Use it for \
                          'what was I working on' questions.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "days": { "type": "integer", "description": "Default 7." },
                    "limit": { "type": "integer", "description": "Default 10." }
                }
            }),
        },
    ]
}

pub fn summarize(name: &str, args: &Value) -> String {
    let text = |key: &str| args.get(key).and_then(|v| v.as_str()).unwrap_or("");
    match name {
        "search" => format!("Searched “{}”", text("query")),
        "grep" => format!("Matched /{}/", text("pattern")),
        "find_notes" => format!("Found notes named “{}”", text("name")),
        "read_note" => format!("Read {}", display_path(text("path"))),
        "list_tags" => "Listed tags".to_string(),
        "notes_with_tag" => format!("Listed #{}", text("tag").trim_start_matches('#')),
        "backlinks" => format!("Checked links to {}", display_path(text("path"))),
        "recent_notes" => "Listed recently edited notes".to_string(),
        _ => format!("Ran {name}"),
    }
}

fn display_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

pub fn run(root: &str, name: &str, args: &Value) -> String {
    let result = match name {
        "search" => vault::search(root, args),
        "grep" => vault::grep(root, args),
        "find_notes" => vault::find_notes(root, args),
        "read_note" => vault::read_note(root, args),
        "list_tags" => vault::list_tags(root),
        "notes_with_tag" => vault::notes_with_tag(root, args),
        "backlinks" => vault::backlinks(root, args),
        "recent_notes" => vault::recent_notes(root, args),
        other => Err(format!("Unknown tool: {other}")),
    };
    match result {
        Ok(text) if text.trim().is_empty() => "No results.".to_string(),
        Ok(text) => cap(&text),
        Err(message) => message,
    }
}

fn cap(text: &str) -> String {
    if text.chars().count() <= MAX_OUTPUT_CHARS {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX_OUTPUT_CHARS).collect();
    out.push_str("\n… (truncated)");
    out
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("Missing required argument: {key}"))
}

fn optional_int(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

// Containment-checked paths are canonical (`\\?\C:\…` on Windows, `/private/var` on macOS), so the raw root may not match.
fn rel_path(root: &str, path: &str) -> String {
    let path = Path::new(path);
    let rel = path
        .strip_prefix(root)
        .ok()
        .map(Path::to_path_buf)
        .or_else(|| {
            let canonical = Path::new(root).canonicalize().ok()?;
            path.strip_prefix(canonical).ok().map(Path::to_path_buf)
        });
    match rel {
        Some(rel) => rel.to_string_lossy().replace('\\', "/"),
        None => path.to_string_lossy().into_owned(),
    }
}

fn note_label(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

fn resolve(root: &str, raw: &str) -> Result<PathBuf, String> {
    let candidate = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        Path::new(root).join(raw)
    };
    ensure_within(root, &candidate.to_string_lossy())
}

#[cfg(test)]
mod tests;
