use crate::ai::config::LlmConfig;
use crate::crypto;
use crate::s3::S3Config;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, specta::Type)]
pub struct AppSettings {
    pub s3: Option<S3Config>,
    #[serde(default)]
    pub attachment_folder: Option<String>,
    #[serde(default)]
    pub auto_sync: Option<bool>,
    #[serde(default)]
    pub conflict_strategy: Option<String>,
    #[serde(default)]
    pub llm: Option<LlmConfig>,
}

/// Save settings encrypted to disk at {vault_path}/.margin/settings.enc
#[tauri::command]
#[specta::specta]
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
    crate::fs::atomic_write(&settings_path, &encrypted)?;

    Ok(())
}

/// Load settings from disk and decrypt
#[tauri::command]
#[specta::specta]
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
#[specta::specta]
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
    if let Some(ref strategy) = settings.conflict_strategy
        && strategy != "local_wins"
        && strategy != "keep_newer"
    {
        return Err(format!("Unknown conflict strategy: {strategy}"));
    }
    if let Some(llm) = &settings.llm {
        crate::ai::config::validate(llm)?;
    }
    Ok(())
}

/// Import settings from an encrypted base64 string
#[tauri::command]
#[specta::specta]
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

#[derive(Serialize, Deserialize, Clone, Debug, specta::Type)]
pub struct WorkspaceTab {
    pub path: String,
    #[serde(rename = "type")]
    pub tab_type: String,
    // `#[serde(default)]` keeps workspace.enc files written before these fields
    // existed loadable — they decode as an unpinned tab with no saved cursor.
    #[serde(default)]
    pub pinned: bool,
    // Which surface the tab was last on: "rich" or "source". Older workspaces
    // decode it as the empty string, which the frontend reads as "rich".
    #[serde(default)]
    pub view_mode: String,
    // ProseMirror document position of the caret, restored on next launch.
    #[serde(default)]
    pub cursor_pos: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, specta::Type)]
pub struct WorkspacePane {
    pub tabs: Vec<WorkspaceTab>,
    pub active_tab_index: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, specta::Type)]
pub struct WorkspaceState {
    pub panes: Vec<WorkspacePane>,
    pub pane_flexes: Vec<f64>,
    #[specta(type = u32)]
    pub active_pane_index: usize,
    pub expanded_folders: Vec<String>,
    pub sidebar_open: bool,
    pub sidebar_width: f64,
    pub sort_order: String,
    // Terminal panel visibility and height. `#[serde(default)]` keeps
    // workspace.enc files written before the panel existed loadable.
    #[serde(default)]
    pub terminal_open: bool,
    #[serde(default)]
    pub terminal_height: f64,
}

/// Save workspace state encrypted to disk at {vault_path}/.margin/workspace.enc
#[tauri::command]
#[specta::specta]
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
    crate::fs::atomic_write(&ws_path, &encrypted)?;

    Ok(())
}

/// Load workspace state from disk and decrypt
#[tauri::command]
#[specta::specta]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Workspaces are encrypted blobs already on users' disks; a new field
    /// without `#[serde(default)]` would make every existing one fail to load.
    #[test]
    fn workspace_tab_decodes_without_newer_fields() {
        let json = r#"{"path":"a.md","type":"markdown","pinned":true}"#;
        let tab: WorkspaceTab = serde_json::from_str(json).unwrap();
        assert_eq!(tab.path, "a.md");
        assert!(tab.pinned);
        assert_eq!(tab.view_mode, "");
        assert_eq!(tab.cursor_pos, None);
    }

    #[test]
    fn workspace_tab_round_trips_view_mode() {
        let tab = WorkspaceTab {
            path: "a.md".into(),
            tab_type: "markdown".into(),
            pinned: false,
            view_mode: "source".into(),
            cursor_pos: Some(12),
        };
        let json = serde_json::to_string(&tab).unwrap();
        let back: WorkspaceTab = serde_json::from_str(&json).unwrap();
        assert_eq!(back.view_mode, "source");
        assert_eq!(back.cursor_pos, Some(12));
    }

    /// A workspace file written before the sidebar dropped its view switcher still
    /// carries `sidebar_view`. It must keep loading: serde ignores the field the
    /// struct no longer declares.
    #[test]
    fn workspace_state_ignores_the_removed_sidebar_view_field() {
        let json = r#"{
            "panes": [
                {
                    "tabs": [{ "path": "/v/a.md", "type": "markdown", "pinned": false, "cursor_pos": 3 }],
                    "active_tab_index": 0
                }
            ],
            "pane_flexes": [1.0],
            "active_pane_index": 0,
            "expanded_folders": ["/v/notes"],
            "sidebar_open": true,
            "sidebar_width": 280.0,
            "sidebar_view": "files",
            "sort_order": "name"
        }"#;

        let state: WorkspaceState = serde_json::from_str(json).expect("old payload must load");

        assert_eq!(state.sidebar_width, 280.0);
        assert_eq!(state.expanded_folders, vec!["/v/notes".to_string()]);
        assert_eq!(state.panes[0].tabs[0].tab_type, "markdown");
        assert_eq!(state.panes[0].tabs[0].cursor_pos, Some(3));
    }
}
