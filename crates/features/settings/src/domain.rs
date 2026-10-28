use std::fmt;

use margin_ask::LlmConfig;
use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
}

impl fmt::Debug for S3Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Config")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("access_key", &"<redacted>")
            .field("secret_key", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTab {
    pub path: String,
    #[serde(rename = "type")]
    pub tab_type: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub view_mode: String,
    #[serde(default)]
    pub cursor_pos: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePane {
    pub tabs: Vec<WorkspaceTab>,
    pub active_tab_index: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub panes: Vec<WorkspacePane>,
    pub pane_flexes: Vec<f64>,
    pub active_pane_index: usize,
    pub expanded_folders: Vec<String>,
    pub sidebar_open: bool,
    pub sidebar_width: f64,
    pub sort_order: String,
    #[serde(default)]
    pub terminal_open: bool,
    #[serde(default)]
    pub terminal_height: f64,
}

pub(crate) fn validate(settings: &AppSettings) -> Result<(), Error> {
    if let Some(s3) = &settings.s3 {
        if s3.endpoint.trim().is_empty() {
            return Err(Error::Invalid("S3 endpoint must not be empty".to_owned()));
        }
        if s3.bucket.trim().is_empty() {
            return Err(Error::Invalid(
                "S3 bucket name must not be empty".to_owned(),
            ));
        }
        if s3.region.trim().is_empty() {
            return Err(Error::Invalid("S3 region must not be empty".to_owned()));
        }
        if s3.access_key.trim().is_empty() {
            return Err(Error::Invalid("S3 access key must not be empty".to_owned()));
        }
        if s3.secret_key.trim().is_empty() {
            return Err(Error::Invalid("S3 secret key must not be empty".to_owned()));
        }
    }
    if let Some(strategy) = &settings.conflict_strategy
        && strategy != "local_wins"
        && strategy != "keep_newer"
    {
        return Err(Error::Invalid(format!(
            "Unknown conflict strategy: {strategy}"
        )));
    }
    if let Some(llm) = &settings.llm {
        llm.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use margin_ask::{ApiFormat, Effort};

    use super::*;

    const ORIGINAL_SETTINGS: &str = r#"{
        "s3": {
            "endpoint": "https://s3.example.test",
            "bucket": "notes",
            "region": "eu-west-1",
            "access_key": "AKIAEXAMPLE",
            "secret_key": "shhh"
        },
        "attachment_folder": "attachments",
        "auto_sync": true,
        "conflict_strategy": "keep_newer",
        "llm": {
            "api_format": "anthropic",
            "base_url": "https://api.anthropic.com",
            "api_key": "sk-secret",
            "model": "claude-3-5",
            "effort": "high"
        }
    }"#;

    #[test]
    fn app_settings_decodes_a_payload_the_tauri_release_wrote() {
        let settings: AppSettings =
            serde_json::from_str(ORIGINAL_SETTINGS).expect("the original payload must load");

        assert_eq!(
            settings.s3,
            Some(S3Config {
                endpoint: "https://s3.example.test".to_owned(),
                bucket: "notes".to_owned(),
                region: "eu-west-1".to_owned(),
                access_key: "AKIAEXAMPLE".to_owned(),
                secret_key: "shhh".to_owned(),
            })
        );
        assert_eq!(settings.attachment_folder.as_deref(), Some("attachments"));
        assert_eq!(settings.auto_sync, Some(true));
        assert_eq!(settings.conflict_strategy.as_deref(), Some("keep_newer"));
        let llm = settings.llm.expect("llm decodes");
        assert_eq!(llm.api_format, ApiFormat::Anthropic);
        assert_eq!(llm.base_url, "https://api.anthropic.com");
        assert_eq!(llm.api_key, "sk-secret");
        assert_eq!(llm.model, "claude-3-5");
        assert_eq!(llm.effort, Some(Effort::High));
        assert_eq!(settings.locale, None);
        assert_eq!(settings.theme, None);
    }

    #[test]
    fn app_settings_decodes_a_payload_without_the_new_ui_fields() {
        let payload = r#"{"s3": null, "auto_sync": false}"#;
        let settings: AppSettings = serde_json::from_str(payload).expect("a partial payload loads");

        assert_eq!(settings.s3, None);
        assert_eq!(settings.auto_sync, Some(false));
        assert_eq!(settings.locale, None);
        assert_eq!(settings.theme, None);
    }

    #[test]
    fn app_settings_reads_and_writes_the_ui_fields() {
        let payload = r#"{"s3": null, "locale": "it", "theme": "dark"}"#;
        let settings: AppSettings = serde_json::from_str(payload).expect("a payload loads");

        assert_eq!(settings.locale.as_deref(), Some("it"));
        assert_eq!(settings.theme.as_deref(), Some("dark"));

        let written: serde_json::Value =
            serde_json::to_value(&settings).expect("settings serialize");
        assert_eq!(written["locale"], "it");
        assert_eq!(written["theme"], "dark");
    }

    #[test]
    fn workspace_tab_decodes_without_newer_fields() {
        let payload = r#"{"path":"a.md","type":"markdown","pinned":true}"#;
        let tab: WorkspaceTab = serde_json::from_str(payload).expect("the payload loads");

        assert_eq!(tab.path, "a.md");
        assert!(tab.pinned);
        assert_eq!(tab.view_mode, "");
        assert_eq!(tab.cursor_pos, None);
    }

    #[test]
    fn workspace_tab_round_trips_view_mode() {
        let tab = WorkspaceTab {
            path: "a.md".to_owned(),
            tab_type: "markdown".to_owned(),
            pinned: false,
            view_mode: "source".to_owned(),
            cursor_pos: Some(12),
        };
        let json = serde_json::to_string(&tab).expect("the tab serializes");
        let back: WorkspaceTab = serde_json::from_str(&json).expect("the tab deserializes");

        assert_eq!(back.view_mode, "source");
        assert_eq!(back.cursor_pos, Some(12));
    }

    #[test]
    fn workspace_state_ignores_the_removed_sidebar_view_field() {
        let payload = r#"{
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

        let state: WorkspaceState = serde_json::from_str(payload).expect("an old payload loads");

        assert!((state.sidebar_width - 280.0).abs() < f64::EPSILON);
        assert_eq!(state.expanded_folders, vec!["/v/notes".to_owned()]);
        assert_eq!(state.panes[0].tabs[0].tab_type, "markdown");
        assert_eq!(state.panes[0].tabs[0].cursor_pos, Some(3));
        assert!(!state.terminal_open);
        assert!(state.terminal_height.abs() < f64::EPSILON);
    }
}
