use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use margin_kernel::{BoxFuture, RelativePath, UnixMillis};
use regex::Regex;

use crate::domain::{AssistantTurn, ChatMessage, LlmConfig, ToolSpec};
use crate::error::Error;

#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchMode {
    All,
    Any,
    Phrase,
}

impl SearchMode {
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "phrase" => Self::Phrase,
            "any" => Self::Any,
            _ => Self::All,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub path: RelativePath,
    pub snippet: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineMatch {
    pub path: RelativePath,
    pub line: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagSummary {
    pub tag: String,
    pub paths: Vec<RelativePath>,
}

impl TagSummary {
    #[must_use]
    pub fn count(&self) -> usize {
        self.paths.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentNote {
    pub path: RelativePath,
    pub modified: UnixMillis,
}

pub trait LanguageModel: Send + Sync {
    fn list_models<'a>(
        &'a self,
        config: &'a LlmConfig,
    ) -> BoxFuture<'a, Result<Vec<String>, Error>>;

    fn stream_chat<'a>(
        &'a self,
        config: &'a LlmConfig,
        system: &'a str,
        messages: &'a [ChatMessage],
        tools: &'a [ToolSpec],
        cancellation: &'a CancellationToken,
        on_text: &'a mut (dyn FnMut(&str) + Send),
    ) -> BoxFuture<'a, Result<AssistantTurn, Error>>;
}

pub trait VaultReader: Send + Sync {
    fn search(&self, query: &str, mode: SearchMode, limit: usize) -> Result<Vec<SearchHit>, Error>;

    fn grep(
        &self,
        pattern: &Regex,
        folder: Option<&RelativePath>,
        max_file_bytes: u64,
        limit: usize,
    ) -> Result<Vec<LineMatch>, Error>;

    fn find_notes(&self, name: &str, limit: usize) -> Result<Vec<RelativePath>, Error>;

    fn read_note(&self, path: &RelativePath) -> Result<String, Error>;

    fn tags(&self) -> Result<Vec<TagSummary>, Error>;

    fn backlinks(&self, path: &RelativePath) -> Result<Vec<RelativePath>, Error>;

    fn recent_notes(&self, since: UnixMillis) -> Result<Vec<RecentNote>, Error>;
}
