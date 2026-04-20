mod walk;
mod search;
mod watch;
mod export;

pub use walk::*;
pub use search::*;
pub use watch::*;
pub use export::*;

use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Convert a `PathBuf` / `OsString` into a `String` with forward slashes
/// so the JS frontend gets consistent separators on every platform.
#[inline]
pub(crate) fn path_to_string(p: std::path::PathBuf) -> String {
    let s = p.into_os_string().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
    #[cfg(target_os = "windows")]
    { s.replace('\\', "/") }
    #[cfg(not(target_os = "windows"))]
    { s }
}

/// Write `content` to `dest` atomically by writing to a sibling temp file
/// and then renaming it into place.
pub(crate) fn atomic_write(dest: &Path, content: &[u8]) -> Result<(), String> {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dest.with_file_name(format!(".margin-write-{}.tmp", n));
    fs::write(&tmp, content).map_err(|e| format!("Failed to write temp file: {e}"))?;
    fs::rename(&tmp, dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed to finalize write: {e}")
    })
}

pub struct WatcherState(pub Mutex<Option<notify::RecommendedWatcher>>);

/// Watches the entire vault directory recursively so the frontend is notified
/// when *any* file changes (including edits by external programs).
pub struct VaultWatcherState(pub Mutex<Option<notify::RecommendedWatcher>>);

/// Stores the active vault root path so security-sensitive handlers
/// (e.g. the localfile:// URI scheme) can verify that a resolved path
/// stays within the vault.
pub struct VaultPathState(pub Mutex<String>);

#[derive(Serialize, Clone)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub path: String,
    /// Seconds since UNIX epoch (file modification time). 0 if unavailable.
    pub modified: u64,
}

/// A single row in the file tree, pre-sorted and depth-annotated.
#[derive(Serialize, Clone)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub modified: u64,
    /// Nesting depth (0 = vault root level).
    pub depth: usize,
}

/// Returned by `read_link_batch`.
#[derive(Serialize)]
pub struct LinkEntry {
    pub path: String,
    pub links: Vec<String>,
}

#[tauri::command]
pub fn set_vault_directory(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        fs::create_dir_all(p).map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let margin_dir = p.join(".margin").join("docs");
    fs::create_dir_all(&margin_dir).map_err(|e| format!("Failed to create .margin/docs: {e}"))?;
    if let Ok(mut vp) = vault_path_state.0.lock() {
        *vp = path.to_string();
    }
    Ok(())
}

#[tauri::command]
pub fn read_file_bytes(path: &str) -> Result<Response, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read file: {e}"))?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub fn write_file_bytes(request: Request) -> Result<(), String> {
    let path = request
        .headers()
        .get("x-path")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing x-path header".to_string())?;
    let data = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(val) => {
            serde_json::from_value::<Vec<u8>>(val.clone())
                .map_err(|e| format!("Invalid body: {e}"))?
        }
    };
    let p = Path::new(path);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    atomic_write(p, &data)
}

#[tauri::command]
pub fn list_directory(path: &str) -> Result<Vec<FsEntry>, String> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Ok(vec![]);
    }
    let mut entries = Vec::new();
    let dir = fs::read_dir(p).map_err(|e| format!("Failed to read directory: {e}"))?;
    for entry in dir.flatten() {
        let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
        if name.starts_with('.') {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        entries.push(FsEntry {
            name,
            is_dir: entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false),
            path: path_to_string(entry.path()),
            modified,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

#[tauri::command]
pub fn delete_entry(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if p.is_dir() {
        fs::remove_dir_all(p).map_err(|e| format!("Failed to delete directory: {e}"))?;
    } else {
        fs::remove_file(p).map_err(|e| format!("Failed to delete file: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn rename_entry(from: &str, to: &str) -> Result<(), String> {
    let to_path = Path::new(to);
    if let Some(parent) = to_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    fs::rename(from, to).map_err(|e| format!("Failed to rename: {e}"))
}

#[tauri::command]
pub fn create_directory(path: &str) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("Failed to create directory: {e}"))
}

#[tauri::command]
pub fn file_exists(path: &str) -> bool {
    Path::new(path).exists()
}

#[tauri::command]
pub fn copy_file(from: &str, to: &str) -> Result<(), String> {
    let to_path = Path::new(to);
    if let Some(parent) = to_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    fs::copy(from, to).map_err(|e| format!("Failed to copy file: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn copy_directory(from: &str, to: &str) -> Result<(), String> {
    let src = Path::new(from);
    let dst = Path::new(to);
    copy_dir_recursive(src, dst)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("Failed to create directory: {e}"))?;

    let mut dirs: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    let mut files_to_copy: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();

    let entries = fs::read_dir(src).map_err(|e| format!("Failed to read directory: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            dirs.push((src_path, dst_path));
        } else {
            files_to_copy.push((src_path, dst_path));
        }
    }

    files_to_copy
        .par_iter()
        .try_for_each(|(s, d)| {
            fs::copy(s, d)
                .map(|_| ())
                .map_err(|e| format!("Failed to copy file: {e}"))
        })?;

    for (src_child, dst_child) in dirs {
        copy_dir_recursive(&src_child, &dst_child)?;
    }
    Ok(())
}

/// Set the modification time of a file to a specific unix timestamp (seconds).
#[tauri::command]
pub fn set_mtime(path: &str, mtime: u64) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err("File does not exist".into());
    }
    let time = filetime::FileTime::from_unix_time(mtime as i64, 0);
    filetime::set_file_mtime(p, time).map_err(|e| format!("Failed to set mtime: {e}"))
}

#[tauri::command]
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
        // explorer.exe requires native backslash paths
        let native = target.to_string_lossy().replace('/', "\\");
        if target.is_dir() {
            Command::new("explorer").arg(&native).status()
        } else {
            // Use raw_arg so the /select,<path> argument isn't quoted by
            // Rust's Command – explorer.exe chokes on the extra quotes and
            // falls back to opening Documents instead of the target file.
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

    let exit_status = status.map_err(|e| format!("Failed to open file manager: {e}"))?;
    // On Windows, explorer.exe always returns exit code 1 even on success,
    // so we skip the exit-code check on that platform.
    #[cfg(not(target_os = "windows"))]
    if !exit_status.success() {
        return Err(format!("File manager exited with status {exit_status}"));
    }
    Ok(())
}

/// Read multiple Markdown files and extract `[[wiki-links]]` from each — all
/// in native Rust using a single IPC call.
#[tauri::command]
pub fn read_link_batch(paths: Vec<String>) -> Vec<LinkEntry> {
    paths
        .into_par_iter()
        .map(|p| {
            let links = extract_wiki_links(Path::new(&p));
            LinkEntry { path: p, links }
        })
        .collect()
}

/// Parse `[[wiki-links]]` (but not `![[image embeds]]`) from a file.
fn extract_wiki_links(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let bytes = content.as_bytes();
    let mut links: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if i > 0 && bytes[i - 1] == b'!' {
                i += 2;
                continue;
            }
            i += 2;
            let start = i;
            while i + 1 < bytes.len() {
                if bytes[i] == b'\n' {
                    break;
                }
                if bytes[i] == b']' && bytes[i + 1] == b']' {
                    let link = content[start..i].trim();
                    if !link.is_empty() {
                        links.push(link.to_string());
                    }
                    i += 2;
                    break;
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    links
}
