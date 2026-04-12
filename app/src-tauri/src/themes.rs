use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Theme {
    pub name: String,
    pub colors: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[derive(Default)]
pub struct ThemeData {
    pub themes: Vec<Theme>,
    pub active_theme: Option<String>,
}


fn themes_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    fs::create_dir_all(&data_dir).map_err(|e| format!("Cannot create app data dir: {e}"))?;
    Ok(data_dir.join("themes.json"))
}

/// Load all saved themes from the app data directory
#[tauri::command]
pub fn load_themes(app: tauri::AppHandle) -> Result<ThemeData, String> {
    let path = themes_path(&app)?;
    if !path.exists() {
        return Ok(ThemeData::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("Read failed: {e}"))?;
    let data: ThemeData =
        serde_json::from_str(&content).map_err(|e| format!("Deserialize failed: {e}"))?;
    Ok(data)
}

/// Save all themes to the app data directory
#[tauri::command]
pub fn save_themes(app: tauri::AppHandle, data: ThemeData) -> Result<(), String> {
    let path = themes_path(&app)?;
    let json = serde_json::to_string_pretty(&data).map_err(|e| format!("Serialize failed: {e}"))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &json).map_err(|e| format!("Write failed: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed to finalise themes write: {e}")
    })?;
    Ok(())
}

/// Export a single theme to a user-chosen path
#[tauri::command]
pub fn export_theme(theme: Theme, dest: String) -> Result<(), String> {
    let json =
        serde_json::to_string_pretty(&theme).map_err(|e| format!("Serialize failed: {e}"))?;
    fs::write(&dest, &json).map_err(|e| format!("Write failed: {e}"))?;
    Ok(())
}

/// Import a theme from a user-chosen JSON file
#[tauri::command]
pub fn import_theme(path: String) -> Result<Theme, String> {
    let content = fs::read_to_string(&path).map_err(|e| format!("Read failed: {e}"))?;
    let theme: Theme =
        serde_json::from_str(&content).map_err(|e| format!("Invalid theme file: {e}"))?;
    if theme.name.trim().is_empty() {
        return Err("Theme name must not be empty".into());
    }
    if theme.colors.is_empty() {
        return Err("Theme must have at least one color".into());
    }
    Ok(theme)
}
