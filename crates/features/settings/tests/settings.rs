use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use margin_ask::{ApiFormat, Effort, LlmConfig};
use margin_crypto::{encrypt, random_key};
use margin_settings::{
    AppSettings, Error, S3Config, Settings, WorkspacePane, WorkspaceState, WorkspaceTab,
};
use margin_test_support::MemoryFileSystem;

const ORIGINAL_SETTINGS_JSON: &str = r#"{"s3":{"endpoint":"https://s3.example.test","bucket":"notes","region":"eu-west-1","access_key":"AKIAEXAMPLE","secret_key":"shhh"},"attachment_folder":"attachments","auto_sync":true,"conflict_strategy":"keep_newer","llm":{"api_format":"anthropic","base_url":"https://api.anthropic.com","api_key":"sk-secret","model":"claude-3-5","effort":"high"}}"#;

const ORIGINAL_WORKSPACE_JSON: &str = r#"{"panes":[{"tabs":[{"path":"/v/a.md","type":"markdown","pinned":false,"cursor_pos":3}],"active_tab_index":0}],"pane_flexes":[1.0],"active_pane_index":0,"expanded_folders":["/v/notes"],"sidebar_open":true,"sidebar_width":280.0,"sidebar_view":"files","sort_order":"name"}"#;

fn settings() -> (Arc<MemoryFileSystem>, Settings) {
    let files = Arc::new(MemoryFileSystem::new());
    let settings = Settings::new(Arc::<MemoryFileSystem>::clone(&files));
    (files, settings)
}

fn sample() -> AppSettings {
    AppSettings {
        s3: Some(S3Config {
            endpoint: "https://s3.example.test".to_owned(),
            bucket: "notes".to_owned(),
            region: "eu-west-1".to_owned(),
            access_key: "AKIAEXAMPLE".to_owned(),
            secret_key: "shhh".to_owned(),
        }),
        attachment_folder: Some("attachments".to_owned()),
        auto_sync: Some(true),
        conflict_strategy: Some("keep_newer".to_owned()),
        llm: Some(LlmConfig {
            api_format: ApiFormat::Anthropic,
            base_url: "https://api.anthropic.com".to_owned(),
            api_key: "sk-secret".to_owned(),
            model: "claude-3-5".to_owned(),
            effort: Some(Effort::High),
        }),
        locale: Some("it".to_owned()),
        theme: Some("dark".to_owned()),
    }
}

fn workspace() -> WorkspaceState {
    WorkspaceState {
        panes: vec![WorkspacePane {
            tabs: vec![WorkspaceTab {
                path: "notes/a.md".to_owned(),
                tab_type: "markdown".to_owned(),
                pinned: true,
                view_mode: "source".to_owned(),
                cursor_pos: Some(3),
            }],
            active_tab_index: 0,
        }],
        pane_flexes: vec![1.0],
        active_pane_index: 0,
        expanded_folders: vec!["notes".to_owned()],
        sidebar_open: true,
        sidebar_width: 280.0,
        sort_order: "name".to_owned(),
        terminal_open: true,
        terminal_height: 200.0,
    }
}

fn as_value<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("serializes")
}

#[test]
fn settings_round_trip_through_the_vault_filesystem() {
    let (files, settings) = settings();
    let key = random_key();

    settings.save(&key, &sample()).expect("saves");
    assert!(files.contents(".margin/settings.enc").is_some());

    let loaded = settings.load(&key).expect("loads").expect("present");
    assert_eq!(as_value(&loaded), as_value(&sample()));
}

#[test]
fn load_returns_none_before_the_first_save() {
    let (_files, settings) = settings();
    assert!(settings.load(&random_key()).expect("loads").is_none());
    assert!(
        settings
            .load_workspace(&random_key())
            .expect("loads")
            .is_none()
    );
}

#[test]
fn loads_a_settings_file_written_by_the_tauri_release() {
    let (files, settings) = settings();
    let key = random_key();
    files.insert(
        ".margin/settings.enc",
        &encrypt(&key, ORIGINAL_SETTINGS_JSON.as_bytes()),
    );

    let loaded = settings.load(&key).expect("loads").expect("present");

    assert_eq!(
        loaded.s3,
        Some(S3Config {
            endpoint: "https://s3.example.test".to_owned(),
            bucket: "notes".to_owned(),
            region: "eu-west-1".to_owned(),
            access_key: "AKIAEXAMPLE".to_owned(),
            secret_key: "shhh".to_owned(),
        })
    );
    assert_eq!(loaded.conflict_strategy.as_deref(), Some("keep_newer"));
    assert_eq!(loaded.locale, None);
    assert_eq!(loaded.theme, None);
    let llm = loaded.llm.expect("llm decodes");
    assert_eq!(llm.model, "claude-3-5");
    assert_eq!(llm.effort, Some(Effort::High));
}

#[test]
fn loads_a_workspace_file_written_by_the_tauri_release() {
    let (files, settings) = settings();
    let key = random_key();
    files.insert(
        ".margin/workspace.enc",
        &encrypt(&key, ORIGINAL_WORKSPACE_JSON.as_bytes()),
    );

    let loaded = settings
        .load_workspace(&key)
        .expect("loads")
        .expect("present");

    assert!((loaded.sidebar_width - 280.0).abs() < f64::EPSILON);
    assert!(loaded.sidebar_open);
    assert_eq!(loaded.expanded_folders, vec!["/v/notes".to_owned()]);
    assert!(!loaded.terminal_open);
    assert!(loaded.terminal_height.abs() < f64::EPSILON);
    let tab = &loaded.panes[0].tabs[0];
    assert_eq!(tab.tab_type, "markdown");
    assert_eq!(tab.view_mode, "");
    assert_eq!(tab.cursor_pos, Some(3));
}

#[test]
fn rejects_a_settings_file_under_the_wrong_key() {
    let (_files, settings) = settings();
    let key = random_key();

    settings.save(&key, &sample()).expect("saves");
    assert!(matches!(
        settings.load(&random_key()),
        Err(Error::Crypto(_))
    ));
}

#[test]
fn workspace_round_trip_through_the_vault_filesystem() {
    let (files, settings) = settings();
    let key = random_key();

    settings.save_workspace(&key, &workspace()).expect("saves");
    assert!(files.contents(".margin/workspace.enc").is_some());

    let loaded = settings
        .load_workspace(&key)
        .expect("loads")
        .expect("present");
    assert_eq!(as_value(&loaded), as_value(&workspace()));
}

#[test]
fn export_import_string_round_trip() {
    let key = random_key();

    let exported = Settings::export_string(&key, &sample()).expect("exports");
    let imported = Settings::import_string(&key, &exported).expect("imports");

    assert_eq!(as_value(&imported), as_value(&sample()));
}

#[test]
fn import_rejects_a_tampered_string() {
    let key = random_key();

    let exported = Settings::export_string(&key, &sample()).expect("exports");
    let mut bytes = B64.decode(&exported).expect("decodes");
    if let Some(byte) = bytes.last_mut() {
        *byte ^= 0xff;
    }
    let tampered = B64.encode(&bytes);

    assert!(matches!(
        Settings::import_string(&key, &tampered),
        Err(Error::Crypto(_))
    ));
    assert!(matches!(
        Settings::import_string(&key, "definitely not base64"),
        Err(Error::Base64(_))
    ));
}

#[test]
fn import_rejects_invalid_settings() {
    let key = random_key();

    let mut invalid = sample();
    invalid.conflict_strategy = Some("nonsense".to_owned());
    let exported = Settings::export_string(&key, &invalid).expect("exports");

    assert!(matches!(
        Settings::import_string(&key, &exported),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn import_validates_the_s3_endpoint() {
    let key = random_key();

    let mut invalid = sample();
    invalid.s3 = Some(S3Config {
        endpoint: "   ".to_owned(),
        bucket: "notes".to_owned(),
        region: "eu-west-1".to_owned(),
        access_key: "key".to_owned(),
        secret_key: "secret".to_owned(),
    });
    let exported = Settings::export_string(&key, &invalid).expect("exports");

    assert!(matches!(
        Settings::import_string(&key, &exported),
        Err(Error::Invalid(_))
    ));
}
