mod domain;
mod error;
mod ports;
mod service;

pub use domain::{
    ApiFormat, AskEvent, AssistantTurn, ChatMessage, Effort, LlmConfig, ToolCall, ToolOutcome,
    ToolSpec, normalize_base_url,
};
pub use error::Error;
pub use ports::{
    CancellationToken, LanguageModel, LineMatch, RecentNote, SearchHit, SearchMode, TagSummary,
    VaultReader,
};
pub use service::AskService;
