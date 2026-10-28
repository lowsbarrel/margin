use std::collections::BTreeMap;
use std::fmt::Write;

use margin_ask::{AssistantTurn, ChatMessage, Error, LlmConfig, ToolSpec};
use reqwest::Client;
use serde_json::{Value, json};

use crate::anthropic_events::{ContentBlock, apply_event, tool_calls};
use crate::fragments::ReasoningBlocks;
use crate::provider::{CHAT_TIMEOUT, ChatCall, MODELS_TIMEOUT, Step, for_each_payload, read_error};

const ANTHROPIC_VERSION: &str = "2023-06-01";
pub(crate) const MAX_TOKENS: u32 = 4096;
pub(crate) const MAX_TOKENS_WITH_THINKING: u32 = 16_384;
const MODELS_PAGE: u32 = 100;
const MAX_MODEL_PAGES: usize = 10;

pub(crate) fn wire_messages(messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = Vec::new();
    for message in messages {
        match message {
            ChatMessage::User(text) => out.push(json!({
                "role": "user",
                "content": [{ "type": "text", "text": text }],
            })),
            ChatMessage::Assistant {
                text,
                calls,
                reasoning,
            } => {
                let mut blocks: Vec<Value> = Vec::with_capacity(reasoning.len() + calls.len() + 1);
                blocks.extend(reasoning.iter().cloned());
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for call in calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": call.args,
                    }));
                }
                if !blocks.is_empty() {
                    out.push(json!({ "role": "assistant", "content": blocks }));
                }
            }
            ChatMessage::ToolResults(results) => {
                if results.is_empty() {
                    continue;
                }
                out.push(json!({
                    "role": "user",
                    "content": results
                        .iter()
                        .map(|result| json!({
                            "type": "tool_result",
                            "tool_use_id": result.call_id,
                            "content": result.content,
                        }))
                        .collect::<Vec<_>>(),
                }));
            }
        }
    }
    out
}

pub(crate) fn request_body(
    config: &LlmConfig,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
) -> Value {
    let mut body = json!({
        "model": config.model,
        "max_tokens": if config.effort.is_some() { MAX_TOKENS_WITH_THINKING } else { MAX_TOKENS },
        "system": system,
        "stream": true,
        "messages": wire_messages(messages),
        "tools": tools
            .iter()
            .map(|tool| json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.parameters,
            }))
            .collect::<Vec<_>>(),
    });
    if let Some(effort) = config.effort {
        body["thinking"] = json!({ "type": "adaptive" });
        body["output_config"] = json!({ "effort": effort.as_str() });
    }
    body
}

pub(crate) async fn stream(call: &mut ChatCall<'_>) -> Result<AssistantTurn, Error> {
    let response = call
        .client
        .post(format!("{}/messages", call.base))
        .timeout(CHAT_TIMEOUT)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("x-api-key", call.config.api_key.trim())
        .json(&request_body(
            call.config,
            call.system,
            call.messages,
            call.tools,
        ))
        .send()
        .await
        .map_err(|error| Error::Failed(format!("Request failed: {error}")))?;
    if !response.status().is_success() {
        return Err(read_error(response).await);
    }

    let cancellation = call.cancellation;
    let on_text = &mut *call.on_text;
    let mut text = String::new();
    let mut blocks: BTreeMap<usize, ContentBlock> = BTreeMap::new();
    let mut reasoning = ReasoningBlocks::new();
    for_each_payload(response, cancellation, |value| {
        if value.get("type").and_then(Value::as_str) == Some("message_stop") {
            return Step::Stop;
        }
        let before = text.len();
        apply_event(value, &mut text, &mut blocks, &mut reasoning);
        if text.len() > before {
            on_text(&text[before..]);
        }
        Step::Continue
    })
    .await?;

    Ok(AssistantTurn {
        text,
        calls: tool_calls(blocks),
        reasoning: reasoning.into_values().map(Value::Object).collect(),
    })
}

pub(crate) async fn list_models(
    client: &Client,
    base: &str,
    key: &str,
) -> Result<Vec<String>, Error> {
    let mut ids = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_MODEL_PAGES {
        let mut url = format!("{base}/models?limit={MODELS_PAGE}");
        if let Some(cursor) = &after {
            url = format!("{url}&after_id={}", percent_encode(cursor));
        }
        let response = client
            .get(url)
            .timeout(MODELS_TIMEOUT)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("x-api-key", key)
            .send()
            .await
            .map_err(|error| Error::Failed(format!("Request failed: {error}")))?;
        if !response.status().is_success() {
            return Err(read_error(response).await);
        }
        let value: Value = response
            .json()
            .await
            .map_err(|error| Error::Failed(format!("Invalid response: {error}")))?;
        if let Some(list) = value.get("data").and_then(Value::as_array) {
            for entry in list {
                if let Some(id) = entry.get("id").and_then(Value::as_str) {
                    ids.push(id.to_owned());
                }
            }
        }
        let more = value
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let last = value
            .get("last_id")
            .and_then(Value::as_str)
            .filter(|cursor| !cursor.is_empty())
            .map(str::to_owned);
        match (more, last) {
            (true, Some(cursor)) => after = Some(cursor),
            _ => break,
        }
    }
    Ok(ids)
}

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() {
            out.push(char::from(byte));
        } else {
            write!(out, "%{byte:02X}").expect("writing into a String cannot fail");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_cursors_encode_every_character_that_is_not_alphanumeric() {
        assert_eq!(percent_encode("claude-3.5"), "claude%2D3%2E5");
        assert_eq!(percent_encode("a_b~c"), "a%5Fb%7Ec");
        assert_eq!(percent_encode("café"), "caf%C3%A9");
        assert_eq!(percent_encode("Opus4"), "Opus4");
    }
}
