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

#[derive(Default)]
pub struct LlmState(pub Mutex<Option<LlmConfig>>);

#[derive(Default)]
pub struct LlmCancelState(pub Mutex<HashMap<String, Arc<AtomicBool>>>);

#[derive(Serialize, Clone, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AskEvent {
    Tool {
        name: String,
        summary: String,
    },
    Delta {
        text: String,
    },
    Done,
    Error {
        status: Option<u16>,
        message: String,
    },
}

#[tauri::command]
#[specta::specta]
pub fn llm_configure(config: LlmConfig, state: State<'_, LlmState>) -> Result<(), String> {
    config::validate(&config)?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(config);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn llm_list_models(config: LlmConfig) -> Result<Vec<String>, String> {
    config::validate_endpoint(&config)?;
    provider::list_models(&config)
        .await
        .map_err(|e| e.message())
}

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

#[tauri::command]
#[specta::specta]
pub fn llm_cancel(request_id: String, state: State<'_, LlmCancelState>) -> Result<(), String> {
    if let Some(flag) = state.0.lock().map_err(|e| e.to_string())?.get(&request_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
