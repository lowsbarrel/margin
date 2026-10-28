use std::collections::VecDeque;
use std::sync::Arc;

use margin_ask::{
    AssistantTurn, CancellationToken, ChatMessage, Error, LanguageModel, LlmConfig, ToolCall,
    ToolSpec,
};
use margin_kernel::BoxFuture;
use parking_lot::Mutex;

#[derive(Debug)]
pub enum ScriptedTurn {
    Reply { text: String, calls: Vec<ToolCall> },
    Fails(Error),
}

impl ScriptedTurn {
    #[must_use]
    pub fn reply(text: &str) -> Self {
        Self::Reply {
            text: text.to_owned(),
            calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn reply_with(text: &str, calls: Vec<ToolCall>) -> Self {
        Self::Reply {
            text: text.to_owned(),
            calls,
        }
    }

    #[must_use]
    pub fn fails(error: Error) -> Self {
        Self::Fails(error)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScriptedLanguageModel {
    turns: Arc<Mutex<VecDeque<ScriptedTurn>>>,
    requests: Arc<Mutex<Vec<Vec<ChatMessage>>>>,
    tools: Arc<Mutex<Vec<ToolSpec>>>,
    models: Vec<String>,
}

impl ScriptedLanguageModel {
    #[must_use]
    pub fn new(turns: Vec<ScriptedTurn>) -> Self {
        Self {
            turns: Arc::new(Mutex::new(turns.into())),
            requests: Arc::new(Mutex::new(Vec::new())),
            tools: Arc::new(Mutex::new(Vec::new())),
            models: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }

    #[must_use]
    pub fn requests(&self) -> Vec<Vec<ChatMessage>> {
        self.requests.lock().clone()
    }

    #[must_use]
    pub fn tools(&self) -> Vec<ToolSpec> {
        self.tools.lock().clone()
    }
}

impl LanguageModel for ScriptedLanguageModel {
    fn list_models<'a>(
        &'a self,
        _config: &'a LlmConfig,
    ) -> BoxFuture<'a, Result<Vec<String>, Error>> {
        let models = self.models.clone();
        Box::pin(async move { Ok(models) })
    }

    fn stream_chat<'a>(
        &'a self,
        _config: &'a LlmConfig,
        _system: &'a str,
        messages: &'a [ChatMessage],
        tools: &'a [ToolSpec],
        _cancellation: &'a CancellationToken,
        on_text: &'a mut (dyn FnMut(&str) + Send),
    ) -> BoxFuture<'a, Result<AssistantTurn, Error>> {
        let turn = self.turns.lock().pop_front();
        self.requests.lock().push(messages.to_vec());
        *self.tools.lock() = tools.to_vec();
        Box::pin(async move {
            match turn {
                Some(ScriptedTurn::Reply { text, calls }) => {
                    if !text.is_empty() {
                        on_text(&text);
                    }
                    Ok(AssistantTurn {
                        text,
                        calls,
                        reasoning: Vec::new(),
                    })
                }
                Some(ScriptedTurn::Fails(error)) => Err(error),
                None => Err(Error::Failed("the script ran out of turns".to_owned())),
            }
        })
    }
}
