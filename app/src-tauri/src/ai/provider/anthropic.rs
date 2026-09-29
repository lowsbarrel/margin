use super::fragments::{PartialCall, ReasoningBlocks, accumulate_reasoning, finish_calls};
use super::{CHAT_TIMEOUT, Cancel, MODELS_TIMEOUT, Step, client, for_each_payload, read_error};
use crate::ai::chat::{AssistantTurn, ChatMessage, LlmError, ToolCall, ToolSpec};
use crate::ai::config::LlmConfig;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const ANTHROPIC_VERSION: &str = "2023-06-01";
pub(super) const MAX_TOKENS: u32 = 4096;
pub(super) const MAX_TOKENS_WITH_THINKING: u32 = 16_384;
const MODELS_PAGE: u32 = 100;
const MAX_MODEL_PAGES: usize = 10;

pub(super) fn wire_messages(messages: &[ChatMessage]) -> Vec<Value> {
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
                // The API rejects the turn unless thinking blocks come before the tool_use they produced.
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

pub(super) fn request_body(
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
    // Claude pairs an effort with adaptive thinking; either one alone is rejected or ignored.
    if let Some(effort) = config.effort {
        body["thinking"] = json!({ "type": "adaptive" });
        body["output_config"] = json!({ "effort": effort.as_str() });
    }
    body
}

pub(super) async fn stream(
    config: &LlmConfig,
    base: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
    cancel: &Cancel,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<AssistantTurn, LlmError> {
    let response = client(CHAT_TIMEOUT)?
        .post(format!("{base}/messages"))
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("x-api-key", config.api_key.trim())
        .json(&request_body(config, system, messages, tools))
        .send()
        .await
        .map_err(|e| LlmError::Failed(format!("Request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(read_error(response).await);
    }

    let mut text = String::new();
    let mut blocks: BTreeMap<usize, ContentBlock> = BTreeMap::new();
    let mut reasoning = ReasoningBlocks::new();
    for_each_payload(response, cancel, |value| {
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

pub(super) async fn list_models(base: &str, key: &str) -> Result<Vec<String>, LlmError> {
    let mut ids = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_MODEL_PAGES {
        let mut url = format!("{base}/models?limit={MODELS_PAGE}");
        if let Some(cursor) = &after {
            let encoded = utf8_percent_encode(cursor, NON_ALPHANUMERIC);
            url.push_str(&format!("&after_id={encoded}"));
        }
        let response = client(MODELS_TIMEOUT)?
            .get(url)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("x-api-key", key)
            .send()
            .await
            .map_err(|e| LlmError::Failed(format!("Request failed: {e}")))?;
        if !response.status().is_success() {
            return Err(read_error(response).await);
        }
        let value: Value = response
            .json()
            .await
            .map_err(|e| LlmError::Failed(format!("Invalid response: {e}")))?;
        if let Some(list) = value.get("data").and_then(|d| d.as_array()) {
            for entry in list {
                if let Some(id) = entry.get("id").and_then(|v| v.as_str()) {
                    ids.push(id.to_string());
                }
            }
        }
        let more = value
            .get("has_more")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let last = value
            .get("last_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        match (more, last) {
            (true, Some(cursor)) => after = Some(cursor),
            _ => break,
        }
    }
    Ok(ids)
}

#[derive(Default)]
struct ContentBlock {
    id: String,
    name: String,
    args: String,
}

fn apply_event(
    event: &Value,
    text: &mut String,
    blocks: &mut BTreeMap<usize, ContentBlock>,
    reasoning: &mut ReasoningBlocks,
) {
    let kind = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
    match kind {
        "content_block_start" => {
            let Some(block) = event.get("content_block") else {
                return;
            };
            match block.get("type").and_then(|v| v.as_str()) {
                Some("text") => {
                    if let Some(part) = block.get("text").and_then(|v| v.as_str()) {
                        text.push_str(part);
                    }
                }
                Some("thinking" | "redacted_thinking") => {
                    accumulate_reasoning(index as u64, block, reasoning);
                }
                Some("tool_use") => {
                    let slot = blocks.entry(index).or_default();
                    slot.id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    slot.name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                }
                _ => {}
            }
        }
        "content_block_delta" => {
            let Some(delta) = event.get("delta") else {
                return;
            };
            match delta.get("type").and_then(|v| v.as_str()) {
                Some("text_delta") => {
                    if let Some(part) = delta.get("text").and_then(|v| v.as_str()) {
                        text.push_str(part);
                    }
                }
                Some("thinking_delta") => {
                    if let Some(part) = delta.get("thinking") {
                        accumulate_reasoning(index as u64, &json!({ "thinking": part }), reasoning);
                    }
                }
                Some("signature_delta") => {
                    if let Some(signature) = delta.get("signature") {
                        accumulate_reasoning(
                            index as u64,
                            &json!({ "signature": signature }),
                            reasoning,
                        );
                    }
                }
                Some("input_json_delta") => {
                    if let Some(part) = delta.get("partial_json").and_then(|v| v.as_str()) {
                        blocks.entry(index).or_default().args.push_str(part);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn tool_calls(blocks: BTreeMap<usize, ContentBlock>) -> Vec<ToolCall> {
    let calls = blocks
        .into_iter()
        .filter(|(_, block)| !block.name.is_empty())
        .map(|(index, block)| {
            (
                index,
                PartialCall {
                    id: block.id,
                    name: block.name,
                    args: block.args,
                },
            )
        })
        .collect();
    finish_calls(calls)
}

#[cfg(test)]
mod tests;
