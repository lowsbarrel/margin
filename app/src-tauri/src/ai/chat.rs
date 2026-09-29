#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

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
        reasoning: Vec<serde_json::Value>,
    },
    ToolResults(Vec<ToolOutcome>),
}

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: serde_json::Value,
}

#[derive(Default, Debug)]
pub struct AssistantTurn {
    pub text: String,
    pub calls: Vec<ToolCall>,
    pub reasoning: Vec<serde_json::Value>,
}

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
