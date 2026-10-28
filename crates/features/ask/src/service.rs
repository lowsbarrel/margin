use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use margin_kernel::Clock;
use parking_lot::{Mutex, RwLock};
use regex::Regex;
use serde_json::{Value, json};

use crate::domain::{
    AskEvent, AssistantTurn, ChatMessage, LlmConfig, MAX_GREP_FILE_BYTES, MAX_GREP_MATCHES,
    SEARCH_LIMIT, SYSTEM_PROMPT, ToolOutcome, ToolSpec, cap, format_lines, format_recent,
    format_refs, format_search_hits, format_tags, note_path, notes_with_tag, optional_int,
    recent_cutoff, recent_days, recent_limit, required_str, slice_note, summarize,
};
use crate::error::Error;
use crate::ports::{CancellationToken, LanguageModel, SearchMode, VaultReader};

const MAX_TOOL_ROUNDS: usize = 12;

pub struct AskService {
    model: Arc<dyn LanguageModel>,
    vault: Arc<dyn VaultReader>,
    clock: Arc<dyn Clock>,
    config: RwLock<Option<LlmConfig>>,
    requests: Mutex<HashMap<String, CancellationToken>>,
}

impl AskService {
    #[must_use]
    pub fn new(
        model: Arc<dyn LanguageModel>,
        vault: Arc<dyn VaultReader>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            model,
            vault,
            clock,
            config: RwLock::new(None),
            requests: Mutex::new(HashMap::new()),
        }
    }

    pub fn configure(&self, config: LlmConfig) -> Result<(), Error> {
        config.validate()?;
        *self.config.write() = Some(config);
        Ok(())
    }

    pub async fn list_models(&self, config: &LlmConfig) -> Result<Vec<String>, Error> {
        config.validate_endpoint()?;
        self.model.list_models(config).await
    }

    pub async fn ask(
        &self,
        request_id: &str,
        question: &str,
        on_event: &mut (dyn FnMut(AskEvent) + Send),
    ) -> Result<(), Error> {
        if question.trim().is_empty() {
            return Err(Error::EmptyQuestion);
        }
        let config = self.config.read().clone().ok_or(Error::NotConfigured)?;
        let cancellation = CancellationToken::new();
        self.requests
            .lock()
            .insert(request_id.to_owned(), cancellation.clone());
        let result = self
            .answer(&config, &cancellation, question, on_event)
            .await;
        self.requests.lock().remove(request_id);
        match result {
            Ok(()) => {
                on_event(AskEvent::Done);
                Ok(())
            }
            Err(Error::Cancelled) => Ok(()),
            Err(error) => {
                on_event(AskEvent::Failed(error));
                Ok(())
            }
        }
    }

    pub fn cancel(&self, request_id: &str) {
        if let Some(token) = self.requests.lock().get(request_id) {
            token.cancel();
        }
    }

    async fn answer(
        &self,
        config: &LlmConfig,
        cancellation: &CancellationToken,
        question: &str,
        on_event: &mut (dyn FnMut(AskEvent) + Send),
    ) -> Result<(), Error> {
        let specs = specs();
        let mut messages = vec![ChatMessage::User(question.to_owned())];
        for _ in 0..MAX_TOOL_ROUNDS {
            let turn: AssistantTurn = {
                let mut on_text = |text: &str| {
                    on_event(AskEvent::Delta {
                        text: text.to_owned(),
                    });
                };
                self.model
                    .stream_chat(
                        config,
                        SYSTEM_PROMPT,
                        &messages,
                        &specs,
                        cancellation,
                        &mut on_text,
                    )
                    .await?
            };
            if turn.calls.is_empty() {
                return Ok(());
            }
            let mut outcomes = Vec::with_capacity(turn.calls.len());
            for call in &turn.calls {
                if cancellation.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                on_event(AskEvent::Tool {
                    name: call.name.clone(),
                    summary: summarize(&call.name, &call.args),
                });
                let content = match execute(&*self.vault, &*self.clock, &call.name, &call.args) {
                    Ok(text) if text.trim().is_empty() => "No results.".to_owned(),
                    Ok(text) => cap(&text),
                    Err(error) => error.to_string(),
                };
                outcomes.push(ToolOutcome {
                    call_id: call.id.clone(),
                    content,
                });
            }
            messages.push(ChatMessage::Assistant {
                text: turn.text,
                calls: turn.calls,
                reasoning: turn.reasoning,
            });
            messages.push(ChatMessage::ToolResults(outcomes));
        }
        Err(Error::IterationsExhausted)
    }
}

impl fmt::Debug for AskService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AskService")
            .field("config", &self.config.read())
            .field("requests", &self.requests.lock().len())
            .finish_non_exhaustive()
    }
}

fn execute(
    vault: &dyn VaultReader,
    clock: &dyn Clock,
    name: &str,
    args: &Value,
) -> Result<String, Error> {
    match name {
        "search" => {
            let query = required_str(args, "query")?;
            let mode =
                SearchMode::from_name(args.get("mode").and_then(Value::as_str).unwrap_or("all"));
            let hits = vault.search(query, mode, SEARCH_LIMIT)?;
            Ok(format_search_hits(&hits))
        }
        "grep" => {
            let pattern = required_str(args, "pattern")?;
            let expression =
                Regex::new(pattern).map_err(|error| Error::InvalidPattern(error.to_string()))?;
            let folder = args
                .get("folder")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|folder| !folder.is_empty())
                .map(note_path)
                .transpose()?;
            let matches = vault.grep(
                &expression,
                folder.as_ref(),
                MAX_GREP_FILE_BYTES,
                MAX_GREP_MATCHES,
            )?;
            Ok(format_lines(&matches))
        }
        "find_notes" => {
            let query = required_str(args, "name")?;
            let notes = vault.find_notes(query, SEARCH_LIMIT)?;
            Ok(format_refs(&notes))
        }
        "read_note" => {
            let raw = required_str(args, "path")?;
            let path = note_path(raw)?;
            let body = vault.read_note(&path)?;
            slice_note(
                &path,
                &body,
                optional_int(args, "from_line").unwrap_or(1),
                optional_int(args, "to_line"),
            )
        }
        "list_tags" => Ok(format_tags(&vault.tags()?)),
        "notes_with_tag" => {
            let tag = required_str(args, "tag")?;
            Ok(notes_with_tag(&vault.tags()?, tag))
        }
        "backlinks" => {
            let raw = required_str(args, "path")?;
            let path = note_path(raw)?;
            let links = vault.backlinks(&path)?;
            Ok(format_refs(&links))
        }
        "recent_notes" => {
            let now = clock.now();
            let notes = vault.recent_notes(recent_cutoff(now, recent_days(args)))?;
            Ok(format_recent(&notes, now, recent_limit(args)))
        }
        other => Err(Error::UnknownTool(other.to_owned())),
    }
}

fn specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "search",
            description: "Full-text search across every note in the vault. Use this first for most questions. mode 'all' requires every word (default), 'any' requires at least one, 'phrase' matches the exact phrase.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words to look for." },
                    "mode": { "type": "string", "enum": ["all", "any", "phrase"] }
                },
                "required": ["query"]
            }),
        },
        ToolSpec {
            name: "grep",
            description: "Regular-expression search inside note contents, line by line. Use it for exact wording, identifiers or patterns that full-text search would tokenize away.",
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
        ToolSpec {
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
        ToolSpec {
            name: "read_note",
            description: "Read a note, optionally a line range. Paths come from a previous tool result.",
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
        ToolSpec {
            name: "list_tags",
            description: "Every #tag in the vault with how many notes carry it.",
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
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
        ToolSpec {
            name: "backlinks",
            description: "The notes that link to a given note.",
            parameters: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "recent_notes",
            description: "Notes modified in the last N days, most recent first. Use it for 'what was I working on' questions.",
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
