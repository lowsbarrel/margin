use std::time::Duration;

use futures_util::StreamExt;
use margin_ask::{
    ApiFormat, AssistantTurn, CancellationToken, ChatMessage, Error, LanguageModel, LlmConfig,
    ToolSpec, normalize_base_url,
};
use margin_kernel::BoxFuture;
use reqwest::{Client, Response};
use serde_json::Value;

use crate::sse::SseDecoder;
use crate::{anthropic, openai};

pub(crate) const CHAT_TIMEOUT: Duration = Duration::from_secs(180);
pub(crate) const MODELS_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct HttpLanguageModel {
    client: Client,
}

impl Default for HttpLanguageModel {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpLanguageModel {
    #[must_use]
    pub fn new() -> Self {
        let client = Client::builder()
            .build()
            .expect("the HTTP client builds with the bundled TLS backend");
        Self { client }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Continue,
    Stop,
}

pub(crate) struct ChatCall<'call> {
    pub client: &'call Client,
    pub config: &'call LlmConfig,
    pub base: &'call str,
    pub system: &'call str,
    pub messages: &'call [ChatMessage],
    pub tools: &'call [ToolSpec],
    pub cancellation: &'call CancellationToken,
    pub on_text: &'call mut (dyn FnMut(&str) + Send),
}

impl LanguageModel for HttpLanguageModel {
    fn list_models<'a>(
        &'a self,
        config: &'a LlmConfig,
    ) -> BoxFuture<'a, Result<Vec<String>, Error>> {
        Box::pin(async move {
            let base = normalize_base_url(config.api_format, &config.base_url);
            let key = config.api_key.trim();
            let mut ids = match config.api_format {
                ApiFormat::Openai => openai::list_models(&self.client, &base, key).await?,
                ApiFormat::Anthropic => anthropic::list_models(&self.client, &base, key).await?,
            };
            ids.sort();
            ids.dedup();
            if ids.is_empty() {
                return Err(Error::NoModels);
            }
            Ok(ids)
        })
    }

    fn stream_chat<'a>(
        &'a self,
        config: &'a LlmConfig,
        system: &'a str,
        messages: &'a [ChatMessage],
        tools: &'a [ToolSpec],
        cancellation: &'a CancellationToken,
        on_text: &'a mut (dyn FnMut(&str) + Send),
    ) -> BoxFuture<'a, Result<AssistantTurn, Error>> {
        Box::pin(async move {
            let base = normalize_base_url(config.api_format, &config.base_url);
            let mut call = ChatCall {
                client: &self.client,
                config,
                base: &base,
                system,
                messages,
                tools,
                cancellation,
                on_text,
            };
            match config.api_format {
                ApiFormat::Openai => openai::stream(&mut call).await,
                ApiFormat::Anthropic => anthropic::stream(&mut call).await,
            }
        })
    }
}

pub(crate) fn provider_error_message(body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(body).ok();
    if let Some(value) = parsed {
        for pointer in [
            "/error/message",
            "/error/error/message",
            "/message",
            "/detail",
        ] {
            if let Some(text) = value.pointer(pointer).and_then(Value::as_str) {
                return text.to_owned();
            }
        }
        if let Some(text) = value.pointer("/error").and_then(Value::as_str) {
            return text.to_owned();
        }
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "The provider returned an error with no body".to_owned();
    }
    trimmed.chars().take(500).collect()
}

pub(crate) async fn read_error(response: Response) -> Error {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    Error::Http {
        status: Some(status),
        message: provider_error_message(&body),
    }
}

fn stream_error(value: &Value) -> Option<Error> {
    if value.get("type").and_then(Value::as_str) != Some("error") {
        return None;
    }
    let message = value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map_or_else(|| provider_error_message(&value.to_string()), str::to_owned);
    let status = value
        .get("status")
        .and_then(Value::as_u64)
        .and_then(|code| u16::try_from(code).ok());
    Some(Error::Http { status, message })
}

pub(crate) async fn for_each_payload(
    response: Response,
    cancellation: &CancellationToken,
    mut handle: impl FnMut(&Value) -> Step + Send,
) -> Result<(), Error> {
    let mut decoder = SseDecoder::default();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let chunk = chunk.map_err(|error| Error::Failed(format!("Stream failed: {error}")))?;
        for payload in decoder.push(&chunk) {
            if payload.trim() == "[DONE]" {
                return Ok(());
            }
            let Ok(value) = serde_json::from_str::<Value>(&payload) else {
                continue;
            };
            if let Some(error) = stream_error(&value) {
                return Err(error);
            }
            if handle(&value) == Step::Stop {
                return Ok(());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use margin_ask::{Effort, ToolCall, ToolOutcome};
    use serde_json::json;

    use super::*;

    fn config(format: ApiFormat, effort: Option<Effort>) -> LlmConfig {
        LlmConfig {
            api_format: format,
            base_url: "https://x.test/v1".to_owned(),
            api_key: "k".to_owned(),
            model: "m".to_owned(),
            effort,
        }
    }

    fn tools() -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "search",
            description: "d",
            parameters: json!({"type": "object"}),
        }]
    }

    #[test]
    fn effort_reaches_each_request_shape_only_when_it_is_set() {
        let tools = tools();
        let conversation = vec![ChatMessage::User("q".to_owned())];

        let plain = config(ApiFormat::Openai, None);
        let body = openai::request_body(&plain, "sys", &conversation, &tools);
        assert!(body.get("reasoning_effort").is_none());

        let body = openai::request_body(
            &config(ApiFormat::Openai, Some(Effort::High)),
            "sys",
            &conversation,
            &tools,
        );
        assert_eq!(body["reasoning_effort"], "high");

        let body = anthropic::request_body(&plain, "sys", &conversation, &tools);
        assert!(body.get("thinking").is_none());
        assert!(body.get("output_config").is_none());
        assert_eq!(body["max_tokens"], anthropic::MAX_TOKENS);

        let body = anthropic::request_body(
            &config(ApiFormat::Anthropic, Some(Effort::Medium)),
            "sys",
            &conversation,
            &tools,
        );
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "medium");
        assert_eq!(body["max_tokens"], anthropic::MAX_TOKENS_WITH_THINKING);
    }

    #[test]
    fn message_shapes_are_translated_per_provider() {
        let conversation = vec![
            ChatMessage::User("what did I decide?".to_owned()),
            ChatMessage::Assistant {
                text: "Checking.".to_owned(),
                calls: vec![ToolCall {
                    id: "call_1".to_owned(),
                    name: "search".to_owned(),
                    args: json!({"query": "decision"}),
                }],
                reasoning: Vec::new(),
            },
            ChatMessage::ToolResults(vec![ToolOutcome {
                call_id: "call_1".to_owned(),
                content: "no hits".to_owned(),
            }]),
        ];

        let wired = openai::wire_messages("sys", &conversation);
        assert_eq!(wired[0]["role"], "system");
        assert!(wired[2].get("reasoning_details").is_none());
        assert_eq!(wired[2]["tool_calls"][0]["function"]["name"], "search");
        assert_eq!(
            wired[2]["tool_calls"][0]["function"]["arguments"],
            "{\"query\":\"decision\"}"
        );
        assert_eq!(wired[3]["role"], "tool");
        assert_eq!(wired[3]["tool_call_id"], "call_1");

        let wired = anthropic::wire_messages(&conversation);
        assert_eq!(wired.len(), 3, "system is a top-level field, not a message");
        assert_eq!(wired[1]["content"][1]["type"], "tool_use");
        assert_eq!(wired[1]["content"][1]["input"]["query"], "decision");
        assert_eq!(wired[2]["role"], "user");
        assert_eq!(wired[2]["content"][0]["type"], "tool_result");
        assert_eq!(wired[2]["content"][0]["tool_use_id"], "call_1");
    }

    #[test]
    fn provider_error_message_prefers_the_nested_message() {
        assert_eq!(
            provider_error_message("{\"error\":{\"message\":\"bad key\"}}"),
            "bad key"
        );
        assert_eq!(
            provider_error_message("{\"error\":\"overloaded\"}"),
            "overloaded"
        );
        assert_eq!(provider_error_message("  plain text  "), "plain text");
        assert!(!provider_error_message("").is_empty());
    }
}
