use margin_ask::{AssistantTurn, ChatMessage, Error, LlmConfig, ToolSpec};
use reqwest::Client;
use serde_json::{Value, json};

use crate::fragments::{PartialCalls, ReasoningBlocks, accumulate_reasoning, finish_calls};
use crate::provider::{CHAT_TIMEOUT, ChatCall, MODELS_TIMEOUT, Step, for_each_payload, read_error};

pub(crate) fn wire_messages(system: &str, messages: &[ChatMessage]) -> Vec<Value> {
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

pub(crate) fn request_body(
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

pub(crate) async fn stream(call: &mut ChatCall<'_>) -> Result<AssistantTurn, Error> {
    let mut request = call
        .client
        .post(format!("{}/chat/completions", call.base))
        .timeout(CHAT_TIMEOUT)
        .json(&request_body(
            call.config,
            call.system,
            call.messages,
            call.tools,
        ));
    if !call.config.api_key.trim().is_empty() {
        request = request.bearer_auth(call.config.api_key.trim());
    }
    let response = request
        .send()
        .await
        .map_err(|error| Error::Failed(format!("Request failed: {error}")))?;
    if !response.status().is_success() {
        return Err(read_error(response).await);
    }

    let cancellation = call.cancellation;
    let on_text = &mut *call.on_text;
    let mut text = String::new();
    let mut calls = PartialCalls::new();
    let mut reasoning = ReasoningBlocks::new();
    for_each_payload(response, cancellation, |value| {
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

pub(crate) async fn list_models(
    client: &Client,
    base: &str,
    key: &str,
) -> Result<Vec<String>, Error> {
    let mut request = client.get(format!("{base}/models")).timeout(MODELS_TIMEOUT);
    if !key.is_empty() {
        request = request.bearer_auth(key);
    }
    let response = request
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

    let mut ids = Vec::new();
    if let Some(list) = value.get("data").and_then(Value::as_array) {
        for entry in list {
            if let Some(id) = entry.get("id").and_then(Value::as_str) {
                ids.push(id.to_owned());
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
    if let Some(part) = delta.get("content").and_then(Value::as_str) {
        text.push_str(part);
    }
    if let Some(details) = delta.get("reasoning_details").and_then(Value::as_array) {
        for detail in details {
            let index = detail.get("index").and_then(Value::as_u64).unwrap_or(0);
            accumulate_reasoning(index, detail, reasoning);
        }
    }
    let Some(list) = delta.get("tool_calls").and_then(Value::as_array) else {
        return;
    };
    for entry in list {
        let index = entry.get("index").and_then(Value::as_u64).unwrap_or(0);
        let position = usize::try_from(index).unwrap_or(usize::MAX);
        let slot = calls.entry(position).or_default();
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            id.clone_into(&mut slot.id);
        }
        if let Some(function) = entry.get("function") {
            if let Some(name) = function.get("name").and_then(Value::as_str) {
                name.clone_into(&mut slot.name);
            }
            if let Some(args) = function.get("arguments").and_then(Value::as_str) {
                slot.args.push_str(args);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use margin_ask::ToolOutcome;

    use super::*;

    #[test]
    fn openai_deltas_accumulate_a_tool_call_split_over_chunks() {
        let mut text = String::new();
        let mut calls = PartialCalls::new();
        let mut reasoning = ReasoningBlocks::new();

        apply_delta(
            &json!({"content": "Looking", "tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "search", "arguments": "{\"quer"}}]}),
            &mut text,
            &mut calls,
            &mut reasoning,
        );
        apply_delta(
            &json!({"content": " that up", "tool_calls": [{"index": 0, "function": {"arguments": "y\":\"roadmap\"}"}}]}),
            &mut text,
            &mut calls,
            &mut reasoning,
        );

        assert_eq!(text, "Looking that up");
        let finished = finish_calls(calls);
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].name, "search");
        assert_eq!(finished[0].id, "call_1");
        assert_eq!(finished[0].args, json!({"query": "roadmap"}));
    }

    #[test]
    fn openai_keeps_parallel_tool_calls_apart_by_index() {
        let mut text = String::new();
        let mut calls = PartialCalls::new();
        let mut reasoning = ReasoningBlocks::new();
        apply_delta(
            &json!({"tool_calls": [
                {"index": 0, "id": "a", "function": {"name": "search", "arguments": "{}"}},
                {"index": 1, "id": "b", "function": {"name": "list_tags", "arguments": "{}"}}
            ]}),
            &mut text,
            &mut calls,
            &mut reasoning,
        );
        let names: Vec<String> = finish_calls(calls)
            .into_iter()
            .map(|call| call.name)
            .collect();
        assert_eq!(names, vec!["search".to_owned(), "list_tags".to_owned()]);
    }

    #[test]
    fn openai_reasoning_details_accumulate_and_ride_back_with_the_tool_results() {
        let mut text = String::new();
        let mut calls = PartialCalls::new();
        let mut reasoning = ReasoningBlocks::new();

        apply_delta(
            &json!({"reasoning_details": [
                {"type": "reasoning.text", "index": 0, "text": "The user ", "format": "anthropic-claude-v1"}
            ]}),
            &mut text,
            &mut calls,
            &mut reasoning,
        );
        apply_delta(
            &json!({"reasoning_details": [{"index": 0, "text": "asked about ", "signature": "sig-1"}]}),
            &mut text,
            &mut calls,
            &mut reasoning,
        );
        apply_delta(
            &json!({"reasoning_details": [
                {"index": 0, "text": "the roadmap.", "signature": null, "id": null}
            ]}),
            &mut text,
            &mut calls,
            &mut reasoning,
        );
        apply_delta(
            &json!({
                "reasoning_details": [{"type": "reasoning.summary", "index": 1, "summary": "short"}],
                "tool_calls": [{"index": 0, "id": "call_1", "function": {"name": "search", "arguments": "{}"}}],
            }),
            &mut text,
            &mut calls,
            &mut reasoning,
        );

        assert_eq!(text, "", "reasoning is never answer text");
        let turn = AssistantTurn {
            text,
            calls: finish_calls(calls),
            reasoning: reasoning.into_values().map(Value::Object).collect(),
        };
        assert_eq!(turn.reasoning.len(), 2);
        assert_eq!(turn.reasoning[0]["type"], "reasoning.text");
        assert_eq!(
            turn.reasoning[0]["text"],
            "The user asked about the roadmap."
        );
        assert_eq!(turn.reasoning[0]["signature"], "sig-1");
        assert_eq!(turn.reasoning[0]["format"], "anthropic-claude-v1");
        assert_eq!(turn.reasoning[1]["type"], "reasoning.summary");
        assert_eq!(turn.reasoning[1]["summary"], "short");

        let conversation = vec![
            ChatMessage::User("what did I decide?".to_owned()),
            ChatMessage::Assistant {
                text: turn.text,
                calls: turn.calls,
                reasoning: turn.reasoning,
            },
            ChatMessage::ToolResults(vec![ToolOutcome {
                call_id: "call_1".to_owned(),
                content: "no hits".to_owned(),
            }]),
        ];
        let openai = wire_messages("sys", &conversation);
        assert_eq!(openai[2]["role"], "assistant");
        assert_eq!(
            openai[2]["reasoning_details"][0]["text"],
            "The user asked about the roadmap."
        );
        assert_eq!(openai[2]["reasoning_details"][0]["signature"], "sig-1");
        assert_eq!(
            openai[2]["reasoning_details"][1]["type"],
            "reasoning.summary"
        );
        assert_eq!(openai[3]["role"], "tool");
    }
}
