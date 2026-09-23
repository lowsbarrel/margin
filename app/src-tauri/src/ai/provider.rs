//! The two wire protocols, behind one streaming call.
//!
//! Everything that touches the network lives here. `stream_chat` translates the
//! neutral conversation in [`super::chat`] into the provider's request shape,
//! consumes its SSE stream, and hands back text deltas plus the tool calls it
//! asked for. The agent loop never sees a provider-specific type.
//!
//! The SSE decoding and the per-event accumulation are pure functions so they can
//! be tested against literal chunk sequences — no mock server, no network.

use super::chat::{AssistantTurn, ChatMessage, LlmError, ToolCall, ToolSpec};
use super::config::{ApiFormat, LlmConfig, normalize_base_url};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Cancellation flag for one in-flight request: set by `llm_cancel`, polled
/// between stream chunks so Esc stops the tokens rather than just the UI.
pub type Cancel = Arc<AtomicBool>;

const CHAT_TIMEOUT: Duration = Duration::from_secs(180);
const MODELS_TIMEOUT: Duration = Duration::from_secs(30);
/// Anthropic requires an explicit ceiling; a long answer with a dozen tool rounds
/// is nowhere near it, so this only bounds a runaway turn.
const MAX_TOKENS: u32 = 4096;
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MODELS_PAGE: u32 = 100;
/// `/v1/models` is paginated; a vault assistant never needs more than this many
/// pages, and a provider that reports `has_more` forever must not spin.
const MAX_MODEL_PAGES: usize = 10;

fn cancelled(cancel: &Cancel) -> bool {
    cancel.load(Ordering::Relaxed)
}

fn client(timeout: Duration) -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| LlmError::Failed(format!("HTTP client failed: {e}")))
}

/// The provider's own words for a failure, dug out of whichever JSON shape it
/// used, falling back to the raw body so an HTML error page is still visible.
pub fn provider_error_message(body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(body).ok();
    if let Some(value) = parsed {
        for pointer in [
            "/error/message",
            "/error/error/message",
            "/message",
            "/detail",
        ] {
            if let Some(text) = value.pointer(pointer).and_then(|v| v.as_str()) {
                return text.to_string();
            }
        }
        if let Some(text) = value.pointer("/error").and_then(|v| v.as_str()) {
            return text.to_string();
        }
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "The provider returned an error with no body".to_string();
    }
    trimmed.chars().take(500).collect()
}

async fn read_error(resp: reqwest::Response) -> LlmError {
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    LlmError::Http {
        status: Some(status),
        message: provider_error_message(&body),
    }
}

/// An SSE stream decoder: feed it whatever bytes arrived, get back the `data:`
/// payloads that are now complete. Handles payloads split across chunks, CRLF,
/// comment lines and multi-line `data:` fields.
#[derive(Default)]
pub struct SseDecoder {
    buffer: String,
    data: String,
}

impl SseDecoder {
    pub fn push(&mut self, chunk: &str) -> Vec<String> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();
        while let Some(newline) = self.buffer.find('\n') {
            let mut line = self.buffer[..newline].to_string();
            self.buffer.drain(..=newline);
            if line.ends_with('\r') {
                line.pop();
            }
            if line.is_empty() {
                // A blank line dispatches the event; a stray blank between
                // events must not produce an empty payload.
                if !self.data.is_empty() {
                    events.push(std::mem::take(&mut self.data));
                }
                continue;
            }
            if line.starts_with(':') {
                continue; // keep-alive comment
            }
            // `event:`/`id:`/`retry:` carry nothing we need: both providers name
            // the event kind inside the JSON payload.
            if let Some(rest) = line.strip_prefix("data:") {
                let value = rest.strip_prefix(' ').unwrap_or(rest);
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(value);
            }
        }
        events
    }
}

/// One tool call being accumulated out of streaming fragments. OpenAI sends the
/// id and name once and the arguments as a long series of string fragments.
#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    args: String,
}

type PartialCalls = BTreeMap<usize, PartialCall>;

fn finish_calls(calls: PartialCalls) -> Vec<ToolCall> {
    calls
        .into_values()
        .map(|call| ToolCall {
            id: call.id,
            name: call.name,
            // A model that streamed no arguments, or truncated ones, leaves this
            // empty; the tool then reports the missing argument to the model.
            args: serde_json::from_str(&call.args).unwrap_or(Value::Null),
        })
        .collect()
}

/// Fold one OpenAI `choices[0].delta` into the turn being built.
fn openai_apply_delta(delta: &Value, text: &mut String, calls: &mut PartialCalls) {
    if let Some(part) = delta.get("content").and_then(|c| c.as_str()) {
        text.push_str(part);
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

/// A content block Anthropic is streaming: either text or an accumulated
/// `tool_use` whose `partial_json` fragments are concatenated per block index.
#[derive(Default)]
struct ContentBlock {
    id: String,
    name: String,
    args: String,
}

/// Fold one Anthropic `content_block_start`/`content_block_delta` event in.
fn anthropic_apply_event(
    event: &Value,
    text: &mut String,
    blocks: &mut BTreeMap<usize, ContentBlock>,
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

/// The provider reported a mid-stream failure in the payload itself (a stream
/// that already returned 200 can still carry an error event).
fn stream_error(value: &Value) -> Option<LlmError> {
    let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if kind != "error" {
        return None;
    }
    let message = value
        .pointer("/error/message")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| provider_error_message(&value.to_string()));
    Some(LlmError::Http {
        status: value
            .get("status")
            .and_then(|v| v.as_u64())
            .map(|s| s as u16),
        message,
    })
}

fn openai_messages(system: &str, messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for message in messages {
        match message {
            ChatMessage::User(text) => out.push(json!({ "role": "user", "content": text })),
            ChatMessage::Assistant { text, calls } => {
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

fn anthropic_messages(messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = Vec::new();
    for message in messages {
        match message {
            ChatMessage::User(text) => out.push(json!({
                "role": "user",
                "content": [{ "type": "text", "text": text }],
            })),
            ChatMessage::Assistant { text, calls } => {
                let mut blocks: Vec<Value> = Vec::with_capacity(calls.len() + 1);
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
                // An assistant turn with nothing in it would be rejected as an
                // empty content array.
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

/// Run one model turn, streaming text to `on_text` as it arrives.
///
/// `on_text` is called with each delta, never with the whole accumulated text, so
/// the caller can forward it straight down the IPC channel.
pub async fn stream_chat(
    config: &LlmConfig,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
    cancel: &Cancel,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<AssistantTurn, LlmError> {
    let base = normalize_base_url(config.api_format, &config.base_url);
    match config.api_format {
        ApiFormat::Openai => {
            openai_stream(config, &base, system, messages, tools, cancel, on_text).await
        }
        ApiFormat::Anthropic => {
            anthropic_stream(config, &base, system, messages, tools, cancel, on_text).await
        }
    }
}

async fn openai_stream(
    config: &LlmConfig,
    base: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
    cancel: &Cancel,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<AssistantTurn, LlmError> {
    let body = json!({
        "model": config.model,
        "stream": true,
        "messages": openai_messages(system, messages),
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

    let mut request = client(CHAT_TIMEOUT)?
        .post(format!("{base}/chat/completions"))
        .json(&body);
    // A keyless local server (Ollama, LM Studio) rejects an empty bearer, so the
    // header is only sent when there is something to send.
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
    let mut decoder = SseDecoder::default();
    let mut stream = response.bytes_stream();
    'stream: while let Some(chunk) = stream.next().await {
        if cancelled(cancel) {
            return Err(LlmError::Cancelled);
        }
        let chunk = chunk.map_err(|e| LlmError::Failed(format!("Stream failed: {e}")))?;
        for payload in decoder.push(&String::from_utf8_lossy(&chunk)) {
            if payload.trim() == "[DONE]" {
                break 'stream;
            }
            let Ok(value) = serde_json::from_str::<Value>(&payload) else {
                continue;
            };
            if let Some(error) = stream_error(&value) {
                return Err(error);
            }
            let Some(delta) = value.pointer("/choices/0/delta") else {
                continue;
            };
            let before = text.len();
            openai_apply_delta(delta, &mut text, &mut calls);
            if text.len() > before {
                on_text(&text[before..]);
            }
        }
    }

    Ok(AssistantTurn {
        text,
        calls: finish_calls(calls),
    })
}

async fn anthropic_stream(
    config: &LlmConfig,
    base: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
    cancel: &Cancel,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<AssistantTurn, LlmError> {
    let body = json!({
        "model": config.model,
        "max_tokens": MAX_TOKENS,
        "system": system,
        "stream": true,
        "messages": anthropic_messages(messages),
        "tools": tools
            .iter()
            .map(|tool| json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.parameters,
            }))
            .collect::<Vec<_>>(),
    });

    let response = client(CHAT_TIMEOUT)?
        .post(format!("{base}/messages"))
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("x-api-key", config.api_key.trim())
        .json(&body)
        .send()
        .await
        .map_err(|e| LlmError::Failed(format!("Request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(read_error(response).await);
    }

    let mut text = String::new();
    let mut blocks: BTreeMap<usize, ContentBlock> = BTreeMap::new();
    let mut decoder = SseDecoder::default();
    let mut stream = response.bytes_stream();
    'stream: while let Some(chunk) = stream.next().await {
        if cancelled(cancel) {
            return Err(LlmError::Cancelled);
        }
        let chunk = chunk.map_err(|e| LlmError::Failed(format!("Stream failed: {e}")))?;
        for payload in decoder.push(&String::from_utf8_lossy(&chunk)) {
            let Ok(value) = serde_json::from_str::<Value>(&payload) else {
                continue;
            };
            if let Some(error) = stream_error(&value) {
                return Err(error);
            }
            if value.get("type").and_then(|v| v.as_str()) == Some("message_stop") {
                break 'stream;
            }
            let before = text.len();
            anthropic_apply_event(&value, &mut text, &mut blocks);
            if text.len() > before {
                on_text(&text[before..]);
            }
        }
    }

    let calls = blocks
        .into_iter()
        // Text blocks land in `blocks` too, as empty entries nothing wrote to.
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

    Ok(AssistantTurn {
        text,
        calls: finish_calls(calls),
    })
}

/// The model ids this endpoint advertises, sorted. Used by the settings form
/// before the config is saved, so it takes a config rather than reading state.
pub async fn list_models(config: &LlmConfig) -> Result<Vec<String>, LlmError> {
    let base = normalize_base_url(config.api_format, &config.base_url);
    let key = config.api_key.trim();
    let mut ids = Vec::new();

    match config.api_format {
        ApiFormat::Openai => {
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
            if let Some(list) = value.get("data").and_then(|d| d.as_array()) {
                for entry in list {
                    if let Some(id) = entry.get("id").and_then(|v| v.as_str()) {
                        ids.push(id.to_string());
                    }
                }
            }
        }
        ApiFormat::Anthropic => {
            let mut after: Option<String> = None;
            for _ in 0..MAX_MODEL_PAGES {
                let mut url = format!("{base}/models?limit={MODELS_PAGE}");
                if let Some(cursor) = &after {
                    let encoded = percent_encoding::utf8_percent_encode(
                        cursor,
                        percent_encoding::NON_ALPHANUMERIC,
                    );
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
        }
    }

    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Err(LlmError::Failed(
            "The endpoint returned no models".to_string(),
        ));
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::chat::ToolOutcome;

    #[test]
    fn decoder_survives_payloads_split_across_chunks() {
        let mut decoder = SseDecoder::default();
        assert!(decoder.push("data: {\"a\"").is_empty());
        // Mid-payload newline inside the JSON is not legal SSE, but a CRLF line
        // ending and a chunk boundary in the middle of a token both are.
        let events = decoder.push(":1}\r\n\r\ndata: [DONE]\n\n");
        assert_eq!(events, vec!["{\"a\":1}".to_string(), "[DONE]".to_string()]);
    }

    #[test]
    fn decoder_keeps_multiline_data_and_skips_comments() {
        let mut decoder = SseDecoder::default();
        let events = decoder.push(": ping\nevent: message\ndata: one\ndata: two\n\n");
        assert_eq!(events, vec!["one\ntwo".to_string()]);
    }

    #[test]
    fn openai_deltas_accumulate_a_tool_call_split_over_chunks() {
        let mut text = String::new();
        let mut calls = PartialCalls::new();

        // The id and name arrive on the first fragment; the arguments dribble in.
        openai_apply_delta(
            &json!({"content": "Looking", "tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "search", "arguments": "{\"quer"}}]}),
            &mut text,
            &mut calls,
        );
        openai_apply_delta(
            &json!({"content": " that up", "tool_calls": [{"index": 0, "function": {"arguments": "y\":\"roadmap\"}"}}]}),
            &mut text,
            &mut calls,
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
        openai_apply_delta(
            &json!({"tool_calls": [
                {"index": 0, "id": "a", "function": {"name": "search", "arguments": "{}"}},
                {"index": 1, "id": "b", "function": {"name": "list_tags", "arguments": "{}"}}
            ]}),
            &mut text,
            &mut calls,
        );
        let names: Vec<String> = finish_calls(calls).into_iter().map(|c| c.name).collect();
        // BTreeMap ordering is by index, so the calls come back in the model's order.
        assert_eq!(names, vec!["search".to_string(), "list_tags".to_string()]);
    }

    #[test]
    fn unparseable_tool_arguments_become_null_rather_than_panicking() {
        let mut text = String::new();
        let mut calls = PartialCalls::new();
        openai_apply_delta(
            &json!({"tool_calls": [{"index": 0, "id": "a", "function": {"name": "grep", "arguments": "{\"pattern\": "}}]}),
            &mut text,
            &mut calls,
        );
        let finished = finish_calls(calls);
        assert!(finished[0].args.is_null());
    }

    #[test]
    fn anthropic_events_build_text_and_tool_use_blocks() {
        let mut text = String::new();
        let mut blocks = BTreeMap::new();

        anthropic_apply_event(
            &json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            &mut text,
            &mut blocks,
        );
        anthropic_apply_event(
            &json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Let me check"}}),
            &mut text,
            &mut blocks,
        );
        anthropic_apply_event(
            &json!({"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "toolu_1", "name": "read_note"}}),
            &mut text,
            &mut blocks,
        );
        anthropic_apply_event(
            &json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"path\":"}}),
            &mut text,
            &mut blocks,
        );
        anthropic_apply_event(
            &json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "\"notes/a.md\"}"}}),
            &mut text,
            &mut blocks,
        );
        // Unrelated events must not disturb the turn.
        anthropic_apply_event(
            &json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}}),
            &mut text,
            &mut blocks,
        );

        assert_eq!(text, "Let me check");
        // Only the tool_use block is tracked; text blocks accumulate into `text`.
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[&1].name, "read_note");
        assert_eq!(blocks[&1].args, "{\"path\":\"notes/a.md\"}");
    }

    #[test]
    fn message_shapes_are_translated_per_provider() {
        let messages = vec![
            ChatMessage::User("what did I decide?".into()),
            ChatMessage::Assistant {
                text: "Checking.".into(),
                calls: vec![ToolCall {
                    id: "call_1".into(),
                    name: "search".into(),
                    args: json!({"query": "decision"}),
                }],
            },
            ChatMessage::ToolResults(vec![ToolOutcome {
                call_id: "call_1".into(),
                content: "no hits".into(),
            }]),
        ];

        let openai = openai_messages("sys", &messages);
        assert_eq!(openai[0]["role"], "system");
        assert_eq!(openai[2]["tool_calls"][0]["function"]["name"], "search");
        // Arguments are a JSON *string* on the wire, not an object.
        assert_eq!(
            openai[2]["tool_calls"][0]["function"]["arguments"],
            "{\"query\":\"decision\"}"
        );
        assert_eq!(openai[3]["role"], "tool");
        assert_eq!(openai[3]["tool_call_id"], "call_1");

        let anthropic = anthropic_messages(&messages);
        assert_eq!(
            anthropic.len(),
            3,
            "system is a top-level field, not a message"
        );
        assert_eq!(anthropic[1]["content"][1]["type"], "tool_use");
        assert_eq!(anthropic[1]["content"][1]["input"]["query"], "decision");
        // Tool results ride in a user message as tool_result blocks.
        assert_eq!(anthropic[2]["role"], "user");
        assert_eq!(anthropic[2]["content"][0]["type"], "tool_result");
        assert_eq!(anthropic[2]["content"][0]["tool_use_id"], "call_1");
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
