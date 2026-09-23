//! Ask-the-vault: the `?` prefix in Spotlight.
//!
//! The configured endpoint is the one place note text leaves the machine without
//! being encrypted first, and it only ever does so because the user configured it
//! explicitly. The API key never returns to the frontend: it is saved inside the
//! encrypted settings file and, for the lifetime of the session, cached here in
//! Rust — the same arrangement as S3 ([`crate::s3::s3_configure`]).
//!
//! Everything else stays local: the model gets read-only tools over the vault's
//! own search index and files, and nothing it produces is written back.

pub mod agent;
pub mod chat;
pub mod config;
pub mod provider;
pub mod tools;

use crate::fs::VaultPathState;
use chat::LlmError;
use config::LlmConfig;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri::ipc::Channel;

/// The configured endpoint, handed over once by the settings form.
#[derive(Default)]
pub struct LlmState(pub Mutex<Option<LlmConfig>>);

/// Cancellation flags for in-flight questions, keyed by request id. Esc sets one;
/// the stream loop polls it between chunks.
#[derive(Default)]
pub struct LlmCancelState(pub Mutex<HashMap<String, Arc<AtomicBool>>>);

/// Everything the palette learns about a running question, in order.
#[derive(Serialize, Clone, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AskEvent {
    /// A tool is about to run, with a one-line human summary of what for.
    Tool {
        name: String,
        summary: String,
    },
    /// A fragment of the answer as the model streams it.
    Delta {
        text: String,
    },
    Done,
    /// The provider refused, or the stream broke.
    Error {
        status: Option<u16>,
        message: String,
    },
}

/// Store the endpoint config in Rust state. Called once when settings load and
/// again on save, so the loop never needs the key from the frontend.
#[tauri::command]
#[specta::specta]
pub fn llm_configure(config: LlmConfig, state: State<'_, LlmState>) -> Result<(), String> {
    config::validate(&config)?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(config);
    Ok(())
}

/// The model ids the endpoint advertises. Takes a config rather than reading
/// state because the settings form calls it before saving.
#[tauri::command]
#[specta::specta]
pub async fn llm_list_models(config: LlmConfig) -> Result<Vec<String>, String> {
    config::validate(&config)?;
    provider::list_models(&config)
        .await
        .map_err(|e| e.message())
}

/// Answer `question` from the vault, streaming [`AskEvent`]s back over `on_event`.
///
/// A provider or network failure arrives as `AskEvent::Error`, not as a rejected
/// promise: the answer may be half-streamed, and the palette needs to keep what
/// it already showed.
#[tauri::command]
#[specta::specta]
pub async fn llm_ask(
    request_id: String,
    question: String,
    on_event: Channel<AskEvent>,
    state: State<'_, LlmState>,
    cancel_state: State<'_, LlmCancelState>,
    vault_path_state: State<'_, VaultPathState>,
) -> Result<(), String> {
    if question.trim().is_empty() {
        return Err("Ask a question first".into());
    }
    let config = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("AI is not configured")?;
    let root = vault_path_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();
    if root.is_empty() {
        return Err("No vault is open".into());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    // The guard is dropped before awaiting: `llm_cancel` runs on another task and
    // has to reach this flag while the answer streams.
    cancel_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(request_id.clone(), cancel.clone());

    let mut session = agent::AskSession {
        config: &config,
        root,
        question,
        cancel,
        on_text: &mut |text| {
            let _ = on_event.send(AskEvent::Delta {
                text: text.to_string(),
            });
        },
        on_tool: &mut |name, summary| {
            let _ = on_event.send(AskEvent::Tool {
                name: name.to_string(),
                summary: summary.to_string(),
            });
        },
    };
    let result = agent::answer(&mut session).await;

    cancel_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&request_id);

    match result {
        Ok(()) => {
            let _ = on_event.send(AskEvent::Done);
            Ok(())
        }
        // Cancelled is a user action, not a failure: the palette has already
        // stopped waiting for this request id.
        Err(LlmError::Cancelled) => Ok(()),
        Err(error) => {
            let _ = on_event.send(AskEvent::Error {
                status: error.status(),
                message: error.message(),
            });
            Ok(())
        }
    }
}

/// Stop a running question. A no-op if the request already finished.
#[tauri::command]
#[specta::specta]
pub fn llm_cancel(request_id: String, state: State<'_, LlmCancelState>) -> Result<(), String> {
    if let Some(flag) = state.0.lock().map_err(|e| e.to_string())?.get(&request_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
