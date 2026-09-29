use super::fragments::{PartialCalls, ReasoningBlocks, accumulate_reasoning, finish_calls};
use super::{CHAT_TIMEOUT, Cancel, MODELS_TIMEOUT, Step, client, for_each_payload, read_error};
use crate::ai::chat::{AssistantTurn, ChatMessage, LlmError, ToolSpec};
use crate::ai::config::LlmConfig;
use serde_json::{Value, json};

pub(super) fn wire_messages(system: &str, messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for message in messages {
        match message {
            ChatMessage::User(text) => out.push(json!({ "role": "user", "content": text })),
            ChatMessage::Assistant {
                text,
                calls,
                reasoning,
            } => {
                let mut entry = json!({
                    "role": "assistant",
                    "content": if text.is_empty() { Value::Null } else { json!(text) },
                });
                if !calls.is_empty() {
                    entry["tool_calls"] = Value::Array(
                        calls
                            .iter()
                            .map(|call| {
                                json!({
                                    "id": call.id,
                                    "type": "function",
                                    "function": {
                                        "name": call.name,
                                        "arguments": call.args.to_string(),
                                    },
                                })
                            })
                            .collect(),
                    );
                }
                if !reasoning.is_empty() {
                    // Router-hosted Anthropic models reject the tool result that follows unless these blocks ride back here.
                    entry["reasoning_details"] = Value::Array(reasoning.clone());
                }
                out.push(entry);
            }
            ChatMessage::ToolResults(results) => {
                for result in results {
                    out.push(json!({
                        "role": "tool",
                        "tool_call_id": result.call_id,
                        "content": result.content,
                    }));
                }
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
        "stream": true,
        "messages": wire_messages(system, messages),
        "tools": tools
            .iter()
            .map(|tool| json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                },
            }))
            .collect::<Vec<_>>(),
    });
    if let Some(effort) = config.effort {
        body["reasoning_effort"] = json!(effort.as_str());
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
    let mut request = client(CHAT_TIMEOUT)?
        .post(format!("{base}/chat/completions"))
        .json(&request_body(config, system, messages, tools));
    // Ollama and LM Studio reject an empty bearer, so a keyless server gets no auth header.
    if !config.api_key.trim().is_empty() {
        request = request.bearer_auth(config.api_key.trim());
    }
    let response = request
        .send()
        .await
        .map_err(|e| LlmError::Failed(format!("Request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(read_error(response).await);
    }

    let mut text = String::new();
    let mut calls = PartialCalls::new();
    let mut reasoning = ReasoningBlocks::new();
    for_each_payload(response, cancel, |value| {
        let Some(delta) = value.pointer("/choices/0/delta") else {
            return Step::Continue;
        };
        let before = text.len();
        apply_delta(delta, &mut text, &mut calls, &mut reasoning);
        if text.len() > before {
            on_text(&text[before..]);
        }
        Step::Continue
    })
    .await?;

    Ok(AssistantTurn {
        text,
        calls: finish_calls(calls),
        reasoning: reasoning.into_values().map(Value::Object).collect(),
    })
}

pub(super) async fn list_models(base: &str, key: &str) -> Result<Vec<String>, LlmError> {
    let mut request = client(MODELS_TIMEOUT)?.get(format!("{base}/models"));
    if !key.is_empty() {
        request = request.bearer_auth(key);
    }
    let response = request
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

    let mut ids = Vec::new();
    if let Some(list) = value.get("data").and_then(|d| d.as_array()) {
        for entry in list {
            if let Some(id) = entry.get("id").and_then(|v| v.as_str()) {
                ids.push(id.to_string());
            }
        }
    }
    Ok(ids)
}

fn apply_delta(
    delta: &Value,
    text: &mut String,
    calls: &mut PartialCalls,
    reasoning: &mut ReasoningBlocks,
) {
    if let Some(part) = delta.get("content").and_then(|c| c.as_str()) {
        text.push_str(part);
    }
    if let Some(details) = delta.get("reasoning_details").and_then(Value::as_array) {
        for detail in details {
            let index = detail.get("index").and_then(Value::as_u64).unwrap_or(0);
            accumulate_reasoning(index, detail, reasoning);
        }
    }
    let Some(list) = delta.get("tool_calls").and_then(|c| c.as_array()) else {
        return;
    };
    for entry in list {
        let index = entry.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
        let slot = calls.entry(index).or_default();
        if let Some(id) = entry.get("id").and_then(|v| v.as_str()) {
            slot.id = id.to_string();
        }
        if let Some(function) = entry.get("function") {
            if let Some(name) = function.get("name").and_then(|v| v.as_str()) {
                slot.name = name.to_string();
            }
            if let Some(args) = function.get("arguments").and_then(|v| v.as_str()) {
                slot.args.push_str(args);
            }
        }
    }
}

#[cfg(test)]
mod tests;
