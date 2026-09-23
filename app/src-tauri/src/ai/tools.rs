//! The read-only tools the answer loop may call.
//!
//! Every tool is vault-contained (paths go through the same containment check as
//! the fs commands) and bounded (result caps, file size limits, walk depth), so a
//! model that asks for "everything" gets a useful sample instead of a megabyte of
//! note text — or a path outside the vault. There is deliberately no write,
//! network or shell tool: the worst a bad answer can do is quote the wrong note.

use crate::fs::ensure_within;
use regex::Regex;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// No single tool result may exceed this; the loop also caps each result so a
/// dozen rounds cannot inflate the prompt without bound.
const MAX_OUTPUT_CHARS: usize = 8000;
const SEARCH_LIMIT: u32 = 20;
/// Lines a single `read_note` may return, and the byte ceiling that overrides it
/// for one very long line.
const MAX_NOTE_LINES: usize = 400;
const MAX_NOTE_BYTES: usize = 40 * 1024;
/// Files larger than this are skipped by `grep` rather than read into memory.
const MAX_GREP_FILE_BYTES: u64 = 1024 * 1024;
const MAX_GREP_MATCHES: usize = 200;
const MAX_TAG_NOTES: usize = 50;
const MAX_RECENT_NOTES: usize = 50;

/// Advertised to the model. Descriptions say what each tool answers, not how it
/// works, because the model only ever reads them once.
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

/// The one-line trace shown in the palette while a tool runs. Built from the
/// arguments alone so it can be emitted *before* the tool executes.
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

/// A failure a tool reports to the *model* (missing argument, unknown path) is
/// not an error of the request: the loop hands it back as the tool result so the
/// model can correct itself. Only infrastructure failures abort the turn, and
/// there are none here — hence `Result<_, String>` never escapes `run`.
pub fn run(root: &str, name: &str, args: &Value) -> String {
    let result = match name {
        "search" => tool_search(root, args),
        "grep" => tool_grep(root, args),
        "find_notes" => tool_find_notes(root, args),
        "read_note" => tool_read_note(root, args),
        "list_tags" => tool_list_tags(root),
        "notes_with_tag" => tool_notes_with_tag(root, args),
        "backlinks" => tool_backlinks(root, args),
        "recent_notes" => tool_recent_notes(root, args),
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

/// Strip the vault prefix so the model reads (and cites) `notes/a.md` rather
/// than an absolute path with a machine name in it. Paths that went through the
/// containment check are canonical (`\\?\C:\…` on Windows, `/private/var` on
/// macOS), so the canonical root is tried when the raw one doesn't match.
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

/// Resolve a note path the model supplied: absolute or vault-relative, then the
/// same containment check the fs commands use.
fn resolve(root: &str, raw: &str) -> Result<PathBuf, String> {
    let candidate = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        Path::new(root).join(raw)
    };
    ensure_within(root, &candidate.to_string_lossy())
}

fn tool_search(root: &str, args: &Value) -> Result<String, String> {
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

fn tool_grep(root: &str, args: &Value) -> Result<String, String> {
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

fn tool_find_notes(root: &str, args: &Value) -> Result<String, String> {
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

fn tool_read_note(root: &str, args: &Value) -> Result<String, String> {
    let raw = required_str(args, "path")?;
    let path = resolve(root, raw)?;
    let body = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read {raw}: {e}"))?;
    let lines: Vec<&str> = body.lines().collect();
    let total = lines.len();

    // 1-based, inclusive, clamped to the file and to the per-call line cap.
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

fn tool_list_tags(root: &str) -> Result<String, String> {
    let tags = crate::index::index_tags(root)?;
    let mut out = String::new();
    for tag in tags {
        out.push_str(&format!("#{} ({} notes)\n", tag.tag, tag.count));
    }
    Ok(out)
}

fn tool_notes_with_tag(root: &str, args: &Value) -> Result<String, String> {
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

fn tool_backlinks(root: &str, args: &Value) -> Result<String, String> {
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

fn tool_recent_notes(root: &str, args: &Value) -> Result<String, String> {
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
    // Newest first: the question behind this tool is "what was I doing".
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A vault whose root is spelled the way the frontend spells it (not
    /// canonicalized), so the containment check is exercised as it runs in the app.
    fn temp_vault(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "margin-ai-test-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("projects")).unwrap();
        std::fs::create_dir_all(dir.join("journal")).unwrap();
        std::fs::write(
            dir.join("projects").join("roadmap.md"),
            "#roadmap\nShip the **editor** rewrite.\nDeferred: sync conflicts.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("projects").join("beta.md"),
            "Links to [[roadmap]].\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("journal").join("today.md"),
            "#journal\nEditor work again.\n",
        )
        .unwrap();
        crate::index::rebuild(&dir.to_string_lossy()).unwrap();
        dir
    }

    #[test]
    fn search_finds_notes_and_cites_them_by_name() {
        let root = temp_vault("search");
        let root = root.to_string_lossy().to_string();

        // "all" requires every word; the phrase is split across notes otherwise.
        let out = run(&root, "search", &json!({"query": "editor rewrite"}));
        assert!(out.contains("[[roadmap]]"), "got {out}");
        assert!(out.contains("projects/roadmap.md"), "got {out}");

        // A word only present in the journal must not come back from an AND query.
        let out = run(&root, "search", &json!({"query": "editor rewrite"}));
        assert!(!out.contains("journal/today.md"), "got {out}");

        let out = run(&root, "search", &json!({"query": "editor", "mode": "any"}));
        assert!(out.contains("journal/today.md"), "got {out}");

        // Phrase mode matches the wording in order, where the word-by-word modes
        // would also match a note that merely contains both words apart.
        let out = run(
            &root,
            "search",
            &json!({"query": "editor rewrite", "mode": "phrase"}),
        );
        assert!(out.contains("[[roadmap]]"), "got {out}");
        let reversed = run(
            &root,
            "search",
            &json!({"query": "rewrite editor", "mode": "phrase"}),
        );
        assert!(!reversed.contains("[[roadmap]]"), "got {reversed}");
    }

    #[test]
    fn read_note_caps_the_range_and_never_escapes_the_vault() {
        let dir = temp_vault("read");
        let root = dir.to_string_lossy().to_string();

        let out = run(&root, "read_note", &json!({"path": "projects/roadmap.md"}));
        assert!(out.contains("Ship the **editor** rewrite."), "got {out}");
        assert!(out.contains("lines 1-"), "got {out}");

        let ranged = run(
            &root,
            "read_note",
            &json!({"path": "projects/roadmap.md", "from_line": 2, "to_line": 2}),
        );
        assert!(ranged.contains("Ship the"), "got {ranged}");
        assert!(!ranged.contains("Deferred:"), "got {ranged}");

        // Absolute paths inside the vault are fine; anything outside is refused.
        let absolute = dir.join("projects").join("roadmap.md");
        let out = run(
            &root,
            "read_note",
            &json!({"path": absolute.to_string_lossy()}),
        );
        assert!(out.contains("Ship the"), "got {out}");

        let outside = std::env::temp_dir().join("margin-ai-outside.md");
        std::fs::write(&outside, "secret").unwrap();
        let out = run(
            &root,
            "read_note",
            &json!({"path": outside.to_string_lossy()}),
        );
        assert!(out.contains("escapes the vault"), "got {out}");
        assert!(!out.contains("secret"), "leaked file contents: {out}");

        let out = run(&root, "read_note", &json!({"path": "../escape.md"}));
        assert!(out.contains("escapes the vault"), "got {out}");

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_file(&outside).ok();
    }

    #[test]
    fn grep_reports_line_numbers_and_respects_a_folder() {
        let dir = temp_vault("grep");
        let root = dir.to_string_lossy().to_string();

        // Regex is case-sensitive, unlike the FTS index — that difference is the
        // reason both tools exist.
        let out = run(
            &root,
            "grep",
            &json!({"pattern": "Editor", "folder": "journal"}),
        );
        // Vault-relative with forward slashes on every platform, even though the
        // folder went through canonicalization.
        assert!(out.starts_with("journal/today.md:2:"), "got {out}");
        assert!(!out.contains("roadmap.md"), "got {out}");

        let out = run(&root, "grep", &json!({"pattern": "^Deferred"}));
        assert!(out.starts_with("projects/roadmap.md:3:"), "got {out}");

        // An invalid pattern is reported to the model, not fatal.
        let out = run(&root, "grep", &json!({"pattern": "("}));
        assert!(out.contains("Invalid regular expression"), "got {out}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tags_links_and_recents_come_from_the_index() {
        let dir = temp_vault("tags");
        let root = dir.to_string_lossy().to_string();

        let out = run(&root, "list_tags", &json!({}));
        assert!(out.contains("#roadmap (1 notes)"), "got {out}");
        assert!(out.contains("#journal (1 notes)"), "got {out}");

        // The leading '#' is optional and matching is case-insensitive.
        let out = run(&root, "notes_with_tag", &json!({"tag": "Roadmap"}));
        assert!(out.contains("[[roadmap]]"), "got {out}");
        assert!(!out.contains("today.md"), "got {out}");

        let out = run(&root, "backlinks", &json!({"path": "projects/roadmap.md"}));
        assert!(out.contains("[[beta]]"), "got {out}");

        let out = run(&root, "recent_notes", &json!({"days": 1, "limit": 5}));
        assert!(out.contains("projects/roadmap.md"), "got {out}");
        assert!(out.contains("today"), "got {out}");

        let out = run(&root, "find_notes", &json!({"name": "road"}));
        assert!(out.contains("[[roadmap]]"), "got {out}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_argument_is_reported_to_the_model() {
        let root = temp_vault("missing");
        let root = root.to_string_lossy().to_string();

        let out = run(&root, "read_note", &json!({}));
        assert!(out.contains("Missing required argument: path"), "got {out}");
        let out = run(&root, "search", &json!({"query": "   "}));
        assert!(
            out.contains("Missing required argument: query"),
            "got {out}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_long_note_is_truncated_rather_than_flooding_the_prompt() {
        let dir = temp_vault("long");
        let long = dir.join("projects").join("long.md");
        std::fs::write(&long, "x".repeat(MAX_OUTPUT_CHARS * 2)).unwrap();
        let root = dir.to_string_lossy().to_string();

        let out = run(&root, "read_note", &json!({"path": "projects/long.md"}));
        assert!(
            out.len() <= MAX_OUTPUT_CHARS + 64,
            "not capped: {}",
            out.len()
        );
        assert!(out.contains("truncated"), "got {}", &out[out.len() - 40..]);

        std::fs::remove_dir_all(&dir).ok();
    }
}
