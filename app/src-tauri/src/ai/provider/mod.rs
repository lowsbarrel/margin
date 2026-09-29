mod anthropic;
mod fragments;
mod openai;
mod sse;

use super::chat::{AssistantTurn, ChatMessage, LlmError, ToolSpec};
use super::config::{ApiFormat, LlmConfig, normalize_base_url};
use futures_util::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub type Cancel = Arc<AtomicBool>;

const CHAT_TIMEOUT: Duration = Duration::from_secs(180);
const MODELS_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) enum Step {
    Continue,
    Stop,
}

fn cancelled(cancel: &Cancel) -> bool {
    cancel.load(Ordering::Relaxed)
}

fn client(timeout: Duration) -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| LlmError::Failed(format!("HTTP client failed: {e}")))
}

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

fn stream_error(value: &Value) -> Option<LlmError> {
    if value.get("type").and_then(|v| v.as_str()) != Some("error") {
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

async fn for_each_payload(
    response: reqwest::Response,
    cancel: &Cancel,
    mut handle: impl FnMut(&Value) -> Step + Send,
) -> Result<(), LlmError> {
    let mut decoder = sse::SseDecoder::default();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancelled(cancel) {
            return Err(LlmError::Cancelled);
        }
        let chunk = chunk.map_err(|e| LlmError::Failed(format!("Stream failed: {e}")))?;
        for payload in decoder.push(&String::from_utf8_lossy(&chunk)) {
            if payload.trim() == "[DONE]" {
                return Ok(());
            }
            let Ok(value) = serde_json::from_str::<Value>(&payload) else {
                continue;
            };
            if let Some(error) = stream_error(&value) {
                return Err(error);
            }
            if let Step::Stop = handle(&value) {
                return Ok(());
            }
        }
    }
    Ok(())
}

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
            openai::stream(config, &base, system, messages, tools, cancel, on_text).await
        }
        ApiFormat::Anthropic => {
            anthropic::stream(config, &base, system, messages, tools, cancel, on_text).await
        }
    }
}

pub async fn list_models(config: &LlmConfig) -> Result<Vec<String>, LlmError> {
    let base = normalize_base_url(config.api_format, &config.base_url);
    let key = config.api_key.trim();
    let mut ids = match config.api_format {
        ApiFormat::Openai => openai::list_models(&base, key).await?,
        ApiFormat::Anthropic => anthropic::list_models(&base, key).await?,
    };
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
mod tests;
