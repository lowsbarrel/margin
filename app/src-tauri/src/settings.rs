use crate::crypto;
use crate::s3::S3Config;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub s3: Option<S3Config>,
    #[serde(default)]
    pub attachment_folder: Option<String>,
    #[serde(default)]
    pub auto_sync: Option<bool>,
    #[serde(default)]
    pub conflict_strategy: Option<String>,
}

/// Save settings encrypted to disk at {vault_path}/.margin/settings.enc
#[tauri::command]
pub fn save_settings(
    vault_path: String,
    encryption_key: Vec<u8>,
    settings: AppSettings,
) -> Result<(), String> {
    let json = serde_json::to_vec(&settings).map_err(|e| format!("Serialize failed: {e}"))?;
    let encrypted = crypto::encrypt_blob(json, encryption_key)?;

    let settings_path = Path::new(&vault_path).join(".margin").join("settings.enc");
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Dir creation failed: {e}"))?;
    }
    // Atomic write: a crash mid-write loses settings but doesn't corrupt them.
    let tmp = settings_path.with_extension("tmp");
    fs::write(&tmp, &encrypted).map_err(|e| format!("Write failed: {e}"))?;
    fs::rename(&tmp, &settings_path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed to finalise settings write: {e}")
    })?;

    Ok(())
}

/// Load settings from disk and decrypt
#[tauri::command]
pub fn load_settings(
    vault_path: String,
    encryption_key: Vec<u8>,
) -> Result<Option<AppSettings>, String> {
    let settings_path = Path::new(&vault_path).join(".margin").join("settings.enc");
    if !settings_path.exists() {
        return Ok(None);
    }

    let encrypted = fs::read(&settings_path).map_err(|e| format!("Read failed: {e}"))?;
    let decrypted = crypto::decrypt_blob(encrypted, encryption_key)?;
    let settings: AppSettings =
        serde_json::from_slice(&decrypted).map_err(|e| format!("Deserialize failed: {e}"))?;

    Ok(Some(settings))
}

/// Export all settings as an encrypted base64 string (portable)
#[tauri::command]
pub fn export_settings_string(
    encryption_key: Vec<u8>,
    settings: AppSettings,
) -> Result<String, String> {
    let json = serde_json::to_vec(&settings).map_err(|e| format!("Serialize failed: {e}"))?;
    let encrypted = crypto::encrypt_blob(json, encryption_key)?;
    Ok(B64.encode(&encrypted))
}

/// Validate that imported settings contain plausible values.
fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if let Some(ref s3) = settings.s3 {
        if s3.endpoint.trim().is_empty() {
            return Err("S3 endpoint must not be empty".into());
        }
        if s3.bucket.trim().is_empty() {
            return Err("S3 bucket name must not be empty".into());
        }
        if s3.region.trim().is_empty() {
            return Err("S3 region must not be empty".into());
        }
        if s3.access_key.trim().is_empty() {
            return Err("S3 access key must not be empty".into());
        }
        if s3.secret_key.trim().is_empty() {
            return Err("S3 secret key must not be empty".into());
        }
    }
    if let Some(ref strategy) = settings.conflict_strategy {
        if strategy != "local_wins" && strategy != "keep_newer" {
            return Err(format!("Unknown conflict strategy: {strategy}"));
        }
    }
    Ok(())
}

/// Import settings from an encrypted base64 string
#[tauri::command]
pub fn import_settings_string(
    encryption_key: Vec<u8>,
    encoded: String,
) -> Result<AppSettings, String> {
    let encrypted = B64
        .decode(encoded.trim())
        .map_err(|e| format!("Invalid base64: {e}"))?;
    let decrypted = crypto::decrypt_blob(encrypted, encryption_key)?;
    let settings: AppSettings =
        serde_json::from_slice(&decrypted).map_err(|e| format!("Deserialize failed: {e}"))?;
    validate_settings(&settings)?;
    Ok(settings)
}

// ─── Workspace state persistence ─────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkspaceTab {
    pub path: String,
    #[serde(rename = "type")]
    pub tab_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkspacePane {
    pub tabs: Vec<WorkspaceTab>,
    pub active_tab_index: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkspaceState {
    pub panes: Vec<WorkspacePane>,
    pub pane_flexes: Vec<f64>,
    pub active_pane_index: usize,
    pub expanded_folders: Vec<String>,
    pub sidebar_open: bool,
    pub sidebar_width: f64,
    pub sidebar_view: String,
    pub sort_order: String,
}

/// Save workspace state encrypted to disk at {vault_path}/.margin/workspace.enc
#[tauri::command]
pub fn save_workspace_state(
    vault_path: String,
    encryption_key: Vec<u8>,
    state: WorkspaceState,
) -> Result<(), String> {
    let json = serde_json::to_vec(&state).map_err(|e| format!("Serialize failed: {e}"))?;
    let encrypted = crypto::encrypt_blob(json, encryption_key)?;

    let ws_path = Path::new(&vault_path).join(".margin").join("workspace.enc");
    if let Some(parent) = ws_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Dir creation failed: {e}"))?;
    }
    let tmp = ws_path.with_extension("tmp");
    fs::write(&tmp, &encrypted).map_err(|e| format!("Write failed: {e}"))?;
    fs::rename(&tmp, &ws_path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed to finalise workspace state write: {e}")
    })?;

    Ok(())
}

/// Load workspace state from disk and decrypt
#[tauri::command]
pub fn load_workspace_state(
    vault_path: String,
    encryption_key: Vec<u8>,
) -> Result<Option<WorkspaceState>, String> {
    let ws_path = Path::new(&vault_path).join(".margin").join("workspace.enc");
    if !ws_path.exists() {
        return Ok(None);
    }

    let encrypted = fs::read(&ws_path).map_err(|e| format!("Read failed: {e}"))?;
    let decrypted = crypto::decrypt_blob(encrypted, encryption_key)?;
    let state: WorkspaceState =
        serde_json::from_slice(&decrypted).map_err(|e| format!("Deserialize failed: {e}"))?;

    Ok(Some(state))
}
