use super::{VaultPathState, ensure_in_vault};
use std::path::Path;
use std::process::Command;

#[tauri::command]
#[specta::specta]
pub fn set_mtime(
    path: &str,
    mtime: u32,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    if !p.exists() {
        return Err("File does not exist".into());
    }
    let time = filetime::FileTime::from_unix_time(mtime as i64, 0);
    filetime::set_file_mtime(p, time).map_err(|e| format!("Failed to set mtime: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn reveal_in_file_manager(path: &str) -> Result<(), String> {
    let target = Path::new(path);
    if !target.exists() {
        return Err("Path does not exist".into());
    }

    #[cfg(target_os = "macos")]
    let status = if target.is_dir() {
        Command::new("open").arg(target).status()
    } else {
        Command::new("open").arg("-R").arg(target).status()
    };

    #[cfg(target_os = "windows")]
    let status = {
        use std::os::windows::process::CommandExt;
        let native = target.to_string_lossy().replace('/', "\\");
        if target.is_dir() {
            Command::new("explorer").arg(&native).status()
        } else {
            // explorer.exe only understands /select with a raw, unquoted path.
            Command::new("explorer")
                .raw_arg(format!("/select,{}", native))
                .status()
        }
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let status = {
        let open_target = if target.is_dir() {
            target
        } else {
            target.parent().unwrap_or(target)
        };
        Command::new("xdg-open").arg(open_target).status()
    };

    #[allow(unused_variables)]
    let exit_status = status.map_err(|e| format!("Failed to open file manager: {e}"))?;
    #[cfg(not(target_os = "windows"))]
    if !exit_status.success() {
        return Err(format!("File manager exited with status {exit_status}"));
    }
    Ok(())
}
