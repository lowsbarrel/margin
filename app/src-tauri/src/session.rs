use crate::crypto;
use aes_gcm_siv::aead::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

/// Legacy single-session format (for migration)
#[derive(Serialize, Deserialize)]
struct LegacySession {
    mnemonic: String,
    vault_path: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VaultProfile {
    pub name: String,
    pub mnemonic: String,
    pub vault_path: String,
}

#[derive(Serialize, Deserialize)]
pub struct VaultProfiles {
    pub profiles: Vec<VaultProfile>,
    pub last_used: Option<String>,
}

/// Get or create a random 32-byte device key stored at {app_data}/device.key.
/// This key never leaves the filesystem and is used to encrypt session data,
/// keeping the mnemonic out of plaintext contexts like localStorage.
fn get_device_key(app: &tauri::AppHandle) -> Result<Vec<u8>, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    fs::create_dir_all(&data_dir).map_err(|e| format!("Cannot create app data dir: {e}"))?;

    let key_path = data_dir.join("device.key");

    if key_path.exists() {
        let key = fs::read(&key_path).map_err(|e| format!("Failed to read device key: {e}"))?;
        if key.len() == 32 {
            return Ok(key);
        }
        // Corrupted key — regenerate. On Windows the file may be marked
        // read-only from a previous successful write; clear that first.
        #[cfg(windows)]
        {
            if let Ok(meta) = fs::metadata(&key_path) {
                let mut p = meta.permissions();
                p.set_readonly(false);
                let _ = fs::set_permissions(&key_path, p);
            }
        }
    }

    let mut key = vec![0u8; 32];
    OsRng.fill_bytes(&mut key);
    // Write atomically via a sibling temp file so a crash mid-write doesn't
    // leave a zero-byte or partial device key that would corrupt all sessions.
    let tmp_key_path = key_path.with_extension("tmp");
    fs::write(&tmp_key_path, &key).map_err(|e| format!("Failed to write device key: {e}"))?;
    // Set permissions on the temp file before renaming so the final file
    // is always protected; rename is the last (atomic) step.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_key_path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to set device key permissions: {e}"))?;
    }
    fs::rename(&tmp_key_path, &key_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_key_path);
        format!("Failed to finalise device key write: {e}")
    })?;
    // Restrict key file to owner-read/write only
    #[cfg(unix)]
    {
        // permissions already set above; re-apply after rename for safety
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(windows)]
    {
        // Mark the key file as read-only so other software cannot accidentally
        // overwrite it. The app data dir is already user-scoped on Windows.
        let mut perms = fs::metadata(&key_path)
            .map_err(|e| format!("Failed to read device key metadata: {e}"))?
            .permissions();
        perms.set_readonly(true);
        fs::set_permissions(&key_path, perms)
            .map_err(|e| format!("Failed to set device key permissions: {e}"))?;
    }
    Ok(key)
}

fn profiles_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    Ok(data_dir.join("profiles.enc"))
}

fn legacy_session_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    Ok(data_dir.join("session.enc"))
}

/// Load profiles, migrating from legacy format if needed
fn load_profiles_internal(app: &tauri::AppHandle) -> Result<VaultProfiles, String> {
    let key = get_device_key(app)?;
    let path = profiles_path(app)?;

    if path.exists() {
        let encrypted = fs::read(&path).map_err(|e| format!("Read failed: {e}"))?;
        let decrypted = crypto::decrypt_blob(encrypted, key)?;
        let profiles: VaultProfiles =
            serde_json::from_slice(&decrypted).map_err(|e| format!("Deserialize failed: {e}"))?;
        return Ok(profiles);
    }

    // Try migrating from legacy session.enc
    let legacy_path = legacy_session_path(app)?;
    if legacy_path.exists() {
        let encrypted = fs::read(&legacy_path).map_err(|e| format!("Read failed: {e}"))?;
        if let Ok(decrypted) = crypto::decrypt_blob(encrypted, key.clone()) {
            if let Ok(legacy) = serde_json::from_slice::<LegacySession>(&decrypted) {
                let folder_name = std::path::Path::new(&legacy.vault_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Vault")
                    .to_string();
                let profile = VaultProfile {
                    name: folder_name,
                    mnemonic: legacy.mnemonic,
                    vault_path: legacy.vault_path,
                };
                let profiles = VaultProfiles {
                    last_used: Some(profile.vault_path.clone()),
                    profiles: vec![profile],
                };
                // Save in new format and remove legacy file
                save_profiles_internal(app, &profiles)?;
                let _ = fs::remove_file(&legacy_path);
                return Ok(profiles);
            }
        }
    }

    Ok(VaultProfiles {
        profiles: vec![],
        last_used: None,
    })
}

fn save_profiles_internal(app: &tauri::AppHandle, profiles: &VaultProfiles) -> Result<(), String> {
    let key = get_device_key(app)?;
    let json = serde_json::to_vec(profiles).map_err(|e| format!("Serialize failed: {e}"))?;
    let encrypted = crypto::encrypt_blob(json, key)?;
    let path = profiles_path(app)?;
    // Write atomically: a crash mid-write to profiles.enc (which holds every
    // vault's encrypted seed phrase) would brick all saved vaults permanently.
    // Write to a sibling .tmp file first, then rename atomically into place.
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &encrypted).map_err(|e| format!("Write failed: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed to finalise profiles write: {e}")
    })?;
    Ok(())
}

/// Load all saved vault profiles
#[tauri::command]
pub fn load_vault_profiles(app: tauri::AppHandle) -> Result<VaultProfiles, String> {
    load_profiles_internal(&app)
}

/// Save or update a vault profile. If a profile with the same vault_path exists, update it.
#[tauri::command]
pub fn save_vault_profile(app: tauri::AppHandle, profile: VaultProfile) -> Result<(), String> {
    let mut data = load_profiles_internal(&app)?;
    if let Some(existing) = data
        .profiles
        .iter_mut()
        .find(|p| p.vault_path == profile.vault_path)
    {
        existing.name = profile.name;
        existing.mnemonic = profile.mnemonic;
    } else {
        data.profiles.push(profile.clone());
    }
    data.last_used = Some(profile.vault_path);
    save_profiles_internal(&app, &data)
}

/// Delete a vault profile by vault_path
#[tauri::command]
pub fn delete_vault_profile(app: tauri::AppHandle, vault_path: String) -> Result<(), String> {
    let mut data = load_profiles_internal(&app)?;
    data.profiles.retain(|p| p.vault_path != vault_path);
    if data.last_used.as_deref() == Some(&vault_path) {
        data.last_used = data.profiles.first().map(|p| p.vault_path.clone());
    }
    save_profiles_internal(&app, &data)
}

/// Legacy compatibility: save_session now saves/updates a profile
#[tauri::command]
pub fn save_session(
    app: tauri::AppHandle,
    mnemonic: String,
    vault_path: String,
) -> Result<(), String> {
    let folder_name = std::path::Path::new(&vault_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Vault")
        .to_string();

    // Check if profile already exists to preserve its name
    let data = load_profiles_internal(&app)?;
    let name = data
        .profiles
        .iter()
        .find(|p| p.vault_path == vault_path)
        .map(|p| p.name.clone())
        .unwrap_or(folder_name);

    let profile = VaultProfile {
        name,
        mnemonic,
        vault_path,
    };
    save_vault_profile(app, profile)
}

/// Legacy compatibility: load_session returns the last-used profile
#[tauri::command]
pub fn load_session(app: tauri::AppHandle) -> Result<Option<VaultProfile>, String> {
    let data = load_profiles_internal(&app)?;
    if let Some(last) = &data.last_used {
        if let Some(profile) = data.profiles.iter().find(|p| &p.vault_path == last) {
            return Ok(Some(profile.clone()));
        }
    }
    Ok(data.profiles.into_iter().next())
}

/// Legacy compatibility: clear_session removes the last-used flag but keeps profiles
#[tauri::command]
pub fn clear_session(app: tauri::AppHandle) -> Result<(), String> {
    let mut data = load_profiles_internal(&app)?;
    data.last_used = None;
    save_profiles_internal(&app, &data)
}
