mod export;
mod search;
mod tags;
mod walk;
mod watch;

pub use export::*;
pub use search::*;
pub use tags::*;
pub use walk::*;
pub use watch::*;

use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Convert a `PathBuf` / `OsString` into a `String` with forward slashes
/// so the JS frontend gets consistent separators on every platform.
#[inline]
pub(crate) fn path_to_string(p: std::path::PathBuf) -> String {
    let s = p
        .into_os_string()
        .into_string()
        .unwrap_or_else(|s| s.to_string_lossy().into_owned());
    #[cfg(target_os = "windows")]
    {
        s.replace('\\', "/")
    }
    #[cfg(not(target_os = "windows"))]
    {
        s
    }
}

/// Write `content` to `dest` atomically AND durably by writing to a sibling
/// temp file, fsyncing the file's data to disk, renaming it into place, and
/// finally fsyncing the parent directory so the rename itself survives a
/// crash/power loss.
///
/// Plain `rename` is atomic with respect to *visibility* but not *durability*:
/// after a power loss the rename can be persisted while the file's data blocks
/// are not, yielding a zero-length/partial file. Every encrypted state file
/// (sync base manifest, settings.enc, profiles.enc, snapshots, themes.json)
/// goes through this path, so a torn write here can silently corrupt sync state.
pub(crate) fn atomic_write(dest: &Path, content: &[u8]) -> Result<(), String> {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dest.with_file_name(format!(".margin-write-{}.tmp", n));

    // Write + fsync the temp file's data before it is renamed into place.
    let write_result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Failed to write temp file: {e}"));
    }

    fs::rename(&tmp, dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed to finalize write: {e}")
    })?;

    // Best-effort fsync of the parent directory so the rename entry is durable.
    // On Windows opening/fsyncing a directory is not supported and is a no-op;
    // ignore any error here — the file's own sync_all already covers its data.
    if let Some(parent) = dest.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

/// Validate that `path` stays inside the active vault. Rejects any `..`
/// component outright, then verifies the target resolves under the canonical
/// vault root.
///
/// For paths that already exist (reads/deletes/renames-from) the canonical form
/// is checked directly. For paths that may not yet exist (writes/creates) the
/// nearest existing ancestor is canonicalized (resolving symlinks) and the
/// remaining tail components are re-appended before the containment check.
///
/// Mirrors the containment check in the `localfile://` URI scheme handler in
/// `lib.rs`. Returns the validated path to operate on.
fn ensure_in_vault(path: &str, vault: &VaultPathState) -> Result<PathBuf, String> {
    let vault_root = vault
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();

    // Before a vault is opened (e.g. the login flow writes `.margin/vault.id`
    // and creates `.margin/` *before* calling set_vault_directory), no
    // containment boundary exists yet. Preserve the prior behavior — and avoid
    // breaking startup — by allowing the operation in that window. Once a vault
    // is set, containment is enforced strictly below. We still reject `..`
    // components unconditionally as a baseline guard.
    let mut had_parent_dir = false;
    let target = Path::new(path);
    for component in target.components() {
        if matches!(component, Component::ParentDir) {
            had_parent_dir = true;
            break;
        }
    }
    if had_parent_dir {
        return Err("Path escapes the vault".into());
    }
    if vault_root.is_empty() {
        return Ok(target.to_path_buf());
    }

    let canonical_vault = Path::new(&vault_root)
        .canonicalize()
        .map_err(|e| format!("Failed to resolve vault root: {e}"))?;

    // Resolve as far as the filesystem allows: canonicalize the deepest existing
    // ancestor and re-attach the not-yet-existing tail. This handles writes to
    // new files inside the vault while still resolving symlinks on the ancestor.
    let mut ancestor = target;
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    let mut resolved = loop {
        match ancestor.canonicalize() {
            Ok(c) => break c,
            Err(_) => match ancestor.parent() {
                Some(parent) => {
                    if let Some(name) = ancestor.file_name() {
                        tail.push(name);
                    }
                    ancestor = parent;
                }
                None => return Err("Failed to resolve path".into()),
            },
        }
    };
    for name in tail.into_iter().rev() {
        resolved.push(name);
    }

    if !resolved.starts_with(&canonical_vault) {
        return Err("Path escapes the vault".into());
    }
    Ok(resolved)
}

pub struct WatcherState(pub Mutex<Option<notify::RecommendedWatcher>>);

/// Watches the entire vault directory recursively so the frontend is notified
/// when *any* file changes (including edits by external programs).
pub struct VaultWatcherState(pub Mutex<Option<notify::RecommendedWatcher>>);

/// Stores the active vault root path so security-sensitive handlers
/// (e.g. the localfile:// URI scheme) can verify that a resolved path
/// stays within the vault.
pub struct VaultPathState(pub Mutex<String>);

#[derive(Serialize, Clone, specta::Type)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub path: String,
    /// Seconds since UNIX epoch (file modification time). 0 if unavailable.
    #[specta(type = u32)]
    pub modified: u64,
}

/// A single row in the file tree, pre-sorted and depth-annotated.
#[derive(Serialize, Clone, specta::Type)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    #[specta(type = u32)]
    pub modified: u64,
    /// Nesting depth (0 = vault root level).
    #[specta(type = u32)]
    pub depth: usize,
}

/// Returned by `read_link_batch`.
#[derive(Serialize, specta::Type)]
pub struct LinkEntry {
    pub path: String,
    pub links: Vec<String>,
}

#[tauri::command]
#[specta::specta]
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
pub fn read_file_bytes(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<Response, String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    let bytes = fs::read(&p).map_err(|e| format!("Failed to read file: {e}"))?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub fn write_file_bytes(
    request: Request,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let path = request
        .headers()
        .get("x-path")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing x-path header".to_string())?;
    let data = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(val) => serde_json::from_value::<Vec<u8>>(val.clone())
            .map_err(|e| format!("Invalid body: {e}"))?,
    };
    let p = ensure_in_vault(path, &vault_path_state)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    atomic_write(&p, &data)
}

#[tauri::command]
#[specta::specta]
pub fn list_directory(path: &str) -> Result<Vec<FsEntry>, String> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Ok(vec![]);
    }
    let mut entries = Vec::new();
    let dir = fs::read_dir(p).map_err(|e| format!("Failed to read directory: {e}"))?;
    for entry in dir.flatten() {
        let name = entry
            .file_name()
            .into_string()
            .unwrap_or_else(|s| s.to_string_lossy().into_owned());
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
#[specta::specta]
pub fn delete_entry(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    if p.is_dir() {
        fs::remove_dir_all(&p).map_err(|e| format!("Failed to delete directory: {e}"))?;
    } else {
        fs::remove_file(&p).map_err(|e| format!("Failed to delete file: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn rename_entry(
    from: &str,
    to: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let from_path = ensure_in_vault(from, &vault_path_state)?;
    let to_path = ensure_in_vault(to, &vault_path_state)?;
    if let Some(parent) = to_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    fs::rename(&from_path, &to_path).map_err(|e| format!("Failed to rename: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn create_directory(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    fs::create_dir_all(&p).map_err(|e| format!("Failed to create directory: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn file_exists(path: &str) -> bool {
    Path::new(path).exists()
}

#[tauri::command]
#[specta::specta]
pub fn copy_file(
    from: &str,
    to: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let from_path = ensure_in_vault(from, &vault_path_state)?;
    let to_path = ensure_in_vault(to, &vault_path_state)?;
    if let Some(parent) = to_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    fs::copy(&from_path, &to_path).map_err(|e| format!("Failed to copy file: {e}"))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn copy_directory(
    from: &str,
    to: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let src = ensure_in_vault(from, &vault_path_state)?;
    let dst = ensure_in_vault(to, &vault_path_state)?;
    copy_dir_recursive(&src, &dst)
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

    files_to_copy.par_iter().try_for_each(|(s, d)| {
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
///
/// `mtime` is a u32 (unix seconds) because specta forbids exporting u64 across
/// the IPC boundary. Unix-second timestamps fit in u32 until 2106; the value is
/// widened to i64 for `filetime` below.
#[tauri::command]
#[specta::specta]
pub fn set_mtime(path: &str, mtime: u32) -> Result<(), String> {
    let p = Path::new(path);
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

    #[allow(unused_variables)]
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
#[specta::specta]
pub fn read_link_batch(paths: Vec<String>) -> Vec<LinkEntry> {
    paths
        .into_par_iter()
        .map(|p| {
            let links = extract_wiki_links(Path::new(&p));
            LinkEntry { path: p, links }
        })
        .collect()
}

/// Parse `[[wiki-links]]` (but not `![[image embeds]]`) from a file, reusing
/// the single shared parser in `text::wiki_links` so the file-based and
/// PM-node-based extractors can never drift on parsing rules.
fn extract_wiki_links(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    crate::text::parse_wiki_links(&content)
        .into_iter()
        .map(|link| link.title)
        .collect()
}
