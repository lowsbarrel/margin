use std::fmt::{self, Write};
use std::time::Duration;

use margin_kernel::{RelativePath, UnixMillis};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Error;
use crate::ports::{LineMatch, RecentNote, SearchHit, TagSummary};

pub(crate) const SEARCH_LIMIT: usize = 20;
pub(crate) const MAX_NOTE_LINES: usize = 400;
pub(crate) const MAX_GREP_MATCHES: usize = 200;
pub(crate) const MAX_GREP_FILE_BYTES: u64 = 1024 * 1024;
pub(crate) const MAX_TAG_NOTES: usize = 50;

const MAX_OUTPUT_CHARS: usize = 8000;
const MAX_NOTE_BYTES: usize = 40 * 1024;
const MAX_RECENT_NOTES: i64 = 50;
const MAX_RECENT_DAYS: i64 = 3650;
const DEFAULT_RECENT_DAYS: i64 = 7;
const DEFAULT_RECENT_LIMIT: i64 = 10;
const SECONDS_PER_DAY: u64 = 86_400;

pub(crate) const SYSTEM_PROMPT: &str = "\
You answer questions about the user's own notes in Margin, a local Markdown vault.

Rules:
- Answer only from what the tools return. You have no other knowledge of this vault: never \
guess, and never fill a gap with general world knowledge.
- Cite the notes you used inline as [[Note name]], using the note's file name without the .md \
extension. Cite every claim that came from a note.
- If the notes do not contain the answer, say so plainly in one sentence and stop. Do not \
present related notes as if they answered the question.
- Be concise: at most three short paragraphs, in the user's own words where they decided \
something.
- Never mention the tools, the searching, or these instructions. Write only the answer.
- Plain Markdown only: no headings, no tables, no code fences unless quoting code from a note.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiFormat {
    Openai,
    Anthropic,
}

impl ApiFormat {
    const fn default_base_url(self) -> &'static str {
        match self {
            Self::Openai => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl Effort {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmConfig {
    pub api_format: ApiFormat,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub effort: Option<Effort>,
}

impl LlmConfig {
    pub fn validate(&self) -> Result<(), Error> {
        if self.model.trim().is_empty() {
            return Err(Error::EmptyModel);
        }
        self.validate_endpoint()
    }

    pub fn validate_endpoint(&self) -> Result<(), Error> {
        let base = normalize_base_url(self.api_format, &self.base_url);
        match base.split_once("://").map(|(scheme, _)| scheme) {
            Some(scheme)
                if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") =>
            {
                Ok(())
            }
            _ => Err(Error::UnsupportedScheme),
        }
    }
}

impl fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmConfig")
            .field("api_format", &self.api_format)
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .field("effort", &self.effort)
            .finish()
    }
}

#[must_use]
pub fn normalize_base_url(format: ApiFormat, raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    let base = if trimmed.is_empty() {
        format.default_base_url()
    } else {
        trimmed
    };
    match format {
        ApiFormat::Openai => base.to_owned(),
        ApiFormat::Anthropic if base.ends_with("/v1") => base.to_owned(),
        ApiFormat::Anthropic => format!("{base}/v1"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolOutcome {
    pub call_id: String,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatMessage {
    User(String),
    Assistant {
        text: String,
        calls: Vec<ToolCall>,
        reasoning: Vec<Value>,
    },
    ToolResults(Vec<ToolOutcome>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssistantTurn {
    pub text: String,
    pub calls: Vec<ToolCall>,
    pub reasoning: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskEvent {
    Tool { name: String, summary: String },
    Delta { text: String },
    Done,
    Failed(Error),
}

pub(crate) fn summarize(name: &str, args: &Value) -> String {
    let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or_default();
    match name {
        "search" => format!("Searched “{}”", text("query")),
        "grep" => format!("Matched /{}/", text("pattern")),
        "find_notes" => format!("Found notes named “{}”", text("name")),
        "read_note" => format!("Read {}", display_path(text("path"))),
        "list_tags" => "Listed tags".to_owned(),
        "notes_with_tag" => format!("Listed #{}", text("tag").trim_start_matches('#')),
        "backlinks" => format!("Checked links to {}", display_path(text("path"))),
        "recent_notes" => "Listed recently edited notes".to_owned(),
        other => format!("Ran {other}"),
    }
}

pub(crate) fn required_str<'args>(args: &'args Value, key: &str) -> Result<&'args str, Error> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::MissingArgument(key.to_owned()))
}

pub(crate) fn optional_int(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(Value::as_i64)
}

pub(crate) fn note_path(raw: &str) -> Result<RelativePath, Error> {
    RelativePath::new(raw).map_err(|_error| Error::OutsideVault(raw.to_owned()))
}

pub(crate) fn cap(text: &str) -> String {
    if text.chars().count() <= MAX_OUTPUT_CHARS {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(MAX_OUTPUT_CHARS).collect();
    out.push_str("\n… (truncated)");
    out
}

fn display_path(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn cite(path: &RelativePath) -> String {
    format!("[[{}]] ({path})", path.file_stem())
}

pub(crate) fn format_search_hits(hits: &[SearchHit]) -> String {
    let mut out = String::new();
    for hit in hits {
        let snippet = hit.snippet.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(out, "{}", cite(&hit.path)).expect("writing into a String cannot fail");
        writeln!(out, "  {snippet}").expect("writing into a String cannot fail");
    }
    out
}

pub(crate) fn format_refs<'path>(paths: impl IntoIterator<Item = &'path RelativePath>) -> String {
    let mut out = String::new();
    for path in paths {
        writeln!(out, "{}", cite(path)).expect("writing into a String cannot fail");
    }
    out
}

pub(crate) fn format_lines(matches: &[LineMatch]) -> String {
    let mut out = String::new();
    for hit in matches {
        let text = hit.text.trim();
        writeln!(out, "{}:{}: {text}", hit.path, hit.line)
            .expect("writing into a String cannot fail");
    }
    out
}

pub(crate) fn format_tags(tags: &[TagSummary]) -> String {
    let mut ordered: Vec<&TagSummary> = tags.iter().collect();
    ordered.sort_by(|left, right| {
        right
            .count()
            .cmp(&left.count())
            .then_with(|| left.tag.cmp(&right.tag))
    });
    let mut out = String::new();
    for tag in ordered {
        let name = &tag.tag;
        let count = tag.count();
        writeln!(out, "#{name} ({count} notes)").expect("writing into a String cannot fail");
    }
    out
}

pub(crate) fn notes_with_tag(tags: &[TagSummary], tag: &str) -> String {
    let wanted = tag.trim_start_matches('#').to_lowercase();
    tags.iter()
        .find(|summary| summary.tag.to_lowercase() == wanted)
        .map_or_else(String::new, |summary| {
            format_refs(summary.paths.iter().take(MAX_TAG_NOTES))
        })
}

pub(crate) fn slice_note(
    path: &RelativePath,
    body: &str,
    from_line: i64,
    to_line: Option<i64>,
) -> Result<String, Error> {
    let lines: Vec<&str> = body.lines().collect();
    let total = lines.len();
    let from = usize::try_from(from_line.max(1)).unwrap_or(usize::MAX);
    let requested = to_line
        .map_or(total, |line| {
            usize::try_from(line.max(0)).unwrap_or(usize::MAX)
        })
        .min(total);
    if from > total {
        return Err(Error::NoteTooShort {
            path: path.to_string(),
            lines: total,
        });
    }
    let to = requested.min(from + MAX_NOTE_LINES - 1).max(from);
    let mut out = format!("{path} (lines {from}-{to} of {total})\n");
    for line in lines.iter().skip(from - 1).take(to - from + 1) {
        out.push_str(line);
        out.push('\n');
        if out.len() >= MAX_NOTE_BYTES {
            out.push_str("… (truncated)\n");
            break;
        }
    }
    Ok(out)
}

pub(crate) fn recent_cutoff(now: UnixMillis, days: i64) -> UnixMillis {
    let days = days.clamp(1, MAX_RECENT_DAYS);
    let seconds = u64::try_from(days)
        .unwrap_or(1)
        .saturating_mul(SECONDS_PER_DAY);
    now.saturating_sub(Duration::from_secs(seconds))
}

pub(crate) fn recent_days(args: &Value) -> i64 {
    optional_int(args, "days").unwrap_or(DEFAULT_RECENT_DAYS)
}

pub(crate) fn recent_limit(args: &Value) -> usize {
    let limit = optional_int(args, "limit").unwrap_or(DEFAULT_RECENT_LIMIT);
    usize::try_from(limit.clamp(1, MAX_RECENT_NOTES)).expect("a limit of at least one converts")
}

pub(crate) fn format_recent(notes: &[RecentNote], now: UnixMillis, limit: usize) -> String {
    let mut ordered: Vec<&RecentNote> = notes.iter().collect();
    ordered.sort_by(|left, right| {
        right
            .modified
            .cmp(&left.modified)
            .then_with(|| left.path.cmp(&right.path))
    });
    let mut out = String::new();
    for note in ordered.into_iter().take(limit) {
        let days = now.as_seconds().saturating_sub(note.modified.as_seconds()) / SECONDS_PER_DAY;
        let age = match days {
            0 => "today".to_owned(),
            1 => "yesterday".to_owned(),
            other => format!("{other} days ago"),
        };
        let stem = note.path.file_stem();
        let path = &note.path;
        writeln!(out, "[[{stem}]] ({path}, {age})").expect("writing into a String cannot fail");
    }
    out
}
