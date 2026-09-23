//! The conversation model both providers are translated into.
//!
//! OpenAI and Anthropic disagree on nearly every wire detail — where the system
//! prompt goes, whether tool results are their own role or content blocks, how a
//! tool call is spelled — but they agree on the *shape* of a conversation. The
//! agent loop only speaks this shape; `provider` translates it twice.

/// One tool invocation the model asked for. `args` is already parsed JSON;
/// a provider that streamed unparseable arguments yields `Value::Null`, which
/// the tool reports back as a missing argument rather than crashing the loop.
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

/// What a tool returned, keyed back to the call that asked for it. Both
/// providers address a result by the call id alone.
#[derive(Clone, Debug)]
pub struct ToolOutcome {
    pub call_id: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub enum ChatMessage {
    User(String),
    Assistant {
        text: String,
        calls: Vec<ToolCall>,
        /// The reasoning blocks the model streamed, in the provider's own shape
        /// (`reasoning_details` objects for one format, thinking blocks for the
        /// other). Opaque here on purpose: replaying them unmodified is what keeps
        /// a tool-result turn acceptable.
        reasoning: Vec<serde_json::Value>,
    },
    /// Tool results, always sent as one message (OpenAI wants one `role:tool`
    /// message per call; Anthropic one user message holding all the blocks).
    ToolResults(Vec<ToolOutcome>),
}

/// A read-only tool advertised to the model. `parameters` is a JSON Schema
/// object; each provider nests it under its own key.
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: serde_json::Value,
}

/// One model turn: whatever text it produced plus the tool calls it wants run.
/// Both can be present at once — a model may explain before calling a tool.
#[derive(Default, Debug)]
pub struct AssistantTurn {
    pub text: String,
    pub calls: Vec<ToolCall>,
    /// Provider-native reasoning blocks, in stream order. Never shown as answer
    /// text; they exist only to be echoed when this turn's tools run.
    pub reasoning: Vec<serde_json::Value>,
}

/// A failed turn. Kept separate from `String` so the HTTP status survives to the
/// UI, where "401" and "connection refused" deserve different words.
#[derive(Debug)]
pub enum LlmError {
    Http {
        status: Option<u16>,
        message: String,
    },
    Cancelled,
    Failed(String),
}

impl LlmError {
    pub fn status(&self) -> Option<u16> {
        match self {
            LlmError::Http { status, .. } => *status,
            _ => None,
        }
    }

    pub fn message(&self) -> String {
        match self {
            LlmError::Http { message, .. } | LlmError::Failed(message) => message.clone(),
            LlmError::Cancelled => "Cancelled".to_string(),
        }
    }
}

impl From<String> for LlmError {
    fn from(value: String) -> Self {
        LlmError::Failed(value)
    }
}

impl From<&str> for LlmError {
    fn from(value: &str) -> Self {
        LlmError::Failed(value.to_string())
    }
}
