mod export;
mod search;
pub(crate) mod tags;
mod trash;
mod walk;
mod watch;

pub use export::*;
pub use search::*;
pub use walk::*;
pub use watch::*;

use rayon::prelude::*;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Normalise path separators to `/` on a `&str`. Unlike [`path_to_string`] this
/// is unconditional: it is applied to path strings that may have been recorded
/// on another platform (a vault path stored in a profile, a path echoed by the
/// webview), where a literal `\` is a separator rather than a filename byte.
pub(crate) fn normalise_slashes(p: &str) -> String {
    p.replace('\\', "/")
}

/// Reject a vault-relative path that is empty, contains a backslash, or has a
/// component that is absolute / `..` / hidden (leading `.`). Guard for paths
/// received over IPC before they are joined onto a base directory.
pub(crate) fn valid_rel_path(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.contains('\\')
        && Path::new(rel).components().all(|c| match c {
            Component::Normal(name) => name
                .to_str()
                .map(|s| !s.is_empty() && !s.starts_with('.'))
                .unwrap_or(false),
            _ => false,
        })
}

/// Strip `vault` from `candidate` when `candidate` lies inside it, returning the
/// vault-relative path with forward slashes (`None` when not inside). Both are
/// normalised first. On Windows the check is case-insensitive — a drive letter
/// may differ in case between the vault root and the path the frontend sends —
/// so a `strip_prefix` miss falls back to a component-wise, case-folded compare.
pub(crate) fn rel_under_vault(vault: &str, candidate: &str) -> Option<String> {
    let vault = vault.trim_end_matches('/');
    let candidate = candidate.trim_end_matches('/');
    let vault_path = Path::new(vault);

    let Ok(rel) = Path::new(candidate).strip_prefix(vault_path) else {
        #[cfg(target_os = "windows")]
        return rel_under_vault_case_folded(vault_path, candidate);
        #[cfg(not(target_os = "windows"))]
        return None;
    };
    if rel.as_os_str().is_empty() {
        return None;
    }
    Some(rel.to_string_lossy().replace('\\', "/"))
}

/// Case-insensitive fallback for [`rel_under_vault`]: Windows drive letters may
/// differ in case between the stored vault root and the path the frontend sends.
#[cfg(target_os = "windows")]
fn rel_under_vault_case_folded(vault_path: &Path, candidate: &str) -> Option<String> {
    let v: Vec<&std::ffi::OsStr> = vault_path.components().map(|c| c.as_os_str()).collect();
    let c: Vec<&std::ffi::OsStr> = Path::new(candidate)
        .components()
        .map(|c| c.as_os_str())
        .collect();
    if c.len() <= v.len()
        || !v
            .iter()
            .zip(&c)
            .all(|(a, b)| a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase())
    {
        return None;
    }
    let rel: PathBuf = c[v.len()..].iter().copied().collect();
    Some(rel.to_string_lossy().replace('\\', "/"))
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
    if let Some(parent) = dest.parent()
        && let Ok(dir) = fs::File::open(parent)
    {
        let _ = dir.sync_all();
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

/// As [`ensure_in_vault`], but keeps the final component as spelled: macOS
/// `realpath` would fold a case-only rename (`note.md` → `Note.md`) back onto the
/// source's on-disk name.
fn ensure_in_vault_keep_name(path: &str, vault: &VaultPathState) -> Result<PathBuf, String> {
    let target = Path::new(path);
    let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
        return Err("Invalid path".into());
    };
    let mut resolved = ensure_in_vault(&parent.to_string_lossy(), vault)?;
    resolved.push(name);
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
    // Warm the full-text search index in the background so the first content
    // search is a fast FTS lookup. Skips unchanged files, so this is cheap on
    // subsequent opens. Best-effort: a failure here never blocks opening a vault.
    let root = path.to_string();
    std::thread::spawn(move || {
        if let Err(e) = crate::index::rebuild(&root) {
            eprintln!("Initial search index build failed: {e}");
        }
    });
    // Expired trash is swept on open rather than on a timer: a vault that is
    // never opened needs no sweeping. Same rule as the index warm-up — a
    // failure here is logged, never surfaced.
    let purge_root = path.to_string();
    std::thread::spawn(move || {
        let removed = trash::purge_older_than(&purge_root, trash::PURGE_AGE_MS);
        if removed > 0 {
            eprintln!("Purged {removed} expired trash item(s)");
        }
    });
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
    atomic_write(&p, &data)?;
    // Keep both derived indexes current without waiting on the watcher's 300ms
    // debounce. The FTS upsert re-indexes only this file — a full `rebuild`
    // would stat every note in the vault just to discover the one that changed.
    crate::index::tree::invalidate();
    crate::index::upsert_path(&vault_root(&vault_path_state), &p);
    Ok(())
}

/// The current vault root, or an empty string when no vault is open. Only used
/// to locate the per-vault index database; an empty root makes the index calls
/// no-ops rather than errors.
fn vault_root(vault_path_state: &tauri::State<'_, VaultPathState>) -> String {
    vault_path_state
        .0
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default()
}

/// Write raw bytes to an arbitrary path **without** the vault containment check.
/// Used only for explicit "save as / export" flows where the destination was
/// chosen by the user through the native save dialog (e.g. exporting a note to
/// PDF onto the Desktop). Mirrors `export_vault_zip`, which likewise writes to a
/// user-picked path outside the vault. Raw-byte command (Request body), so it is
/// registered in `run()` but excluded from the specta bindings.
#[tauri::command]
pub fn save_file_bytes(request: Request) -> Result<(), String> {
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
    let dest = Path::new(path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    atomic_write(dest, &data)
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
    let was_dir = p.is_dir();
    let root = vault_root(&vault_path_state);
    trash::trash_entry(&root, &p)?;
    crate::index::tree::invalidate();
    if was_dir {
        // A directory delete removes an unknown number of notes; a prefix sweep
        // is still one statement, versus re-walking the vault to find them.
        crate::index::remove_prefix(&root, &p);
    } else {
        crate::index::remove_path(&root, &p);
    }
    Ok(())
}

/// Error for a write whose destination is already occupied. Names the entry so
/// the user can tell which one is in the way.
fn occupied_error(to: &Path) -> String {
    let name = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path_to_string(to.to_path_buf()));
    format!("\"{name}\" already exists")
}

/// Whether a rename from `from` to `to` would replace a *different* entry.
///
/// `symlink_metadata` rather than `exists`, so a dangling symlink still counts:
/// a rename would silently replace the link.
fn destination_conflicts(from: &Path, to: &Path) -> bool {
    fs::symlink_metadata(to).is_ok() && !same_entry(from, to)
}

/// Whether `to` is another spelling of `from` itself — the case-only rename
/// (`note.md` → `Note.md`) that must be allowed.
///
/// On a case-insensitive filesystem the destination "exists" from the moment the
/// source does, so a plain existence test would forbid the rename. The names
/// must be equal case-folded *and* the directory must hold no entry whose name
/// is byte-equal to the destination: on a case-sensitive filesystem holding both
/// spellings, that byte-equal entry is a different file and the rename must be
/// refused.
fn same_entry(from: &Path, to: &Path) -> bool {
    if from.parent() != to.parent() {
        return false;
    }
    let (Some(from_name), Some(to_name)) = (from.file_name(), to.file_name()) else {
        return false;
    };
    if from_name == to_name {
        return true;
    }
    if from_name.to_string_lossy().to_lowercase() != to_name.to_string_lossy().to_lowercase() {
        return false;
    }
    match fs::read_dir(from.parent().unwrap_or(Path::new("."))) {
        Ok(entries) => !entries
            .flatten()
            .any(|e| e.file_name().as_os_str() == to_name),
        Err(_) => false,
    }
}

/// Rename one entry, refusing to replace whatever is already at `to`.
fn rename_entry_at(from: &Path, to: &Path) -> Result<(), String> {
    if destination_conflicts(from, to) {
        return Err(occupied_error(to));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    fs::rename(from, to).map_err(|e| format!("Failed to rename: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn rename_entry(
    from: &str,
    to: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let from_path = ensure_in_vault(from, &vault_path_state)?;
    let to_path = ensure_in_vault_keep_name(to, &vault_path_state)?;
    let was_dir = from_path.is_dir();
    rename_entry_at(&from_path, &to_path)?;
    crate::index::tree::invalidate();
    let root = vault_root(&vault_path_state);
    if was_dir {
        // Renaming a folder moves every note under it. Drop the old subtree
        // here; the notes reappear under their new paths on the next rebuild,
        // which the watcher's `vault-fs-changed` already triggers.
        crate::index::remove_prefix(&root, &from_path);
    } else {
        crate::index::remove_path(&root, &from_path);
        crate::index::upsert_path(&root, &to_path);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn create_directory(
    path: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let p = ensure_in_vault(path, &vault_path_state)?;
    fs::create_dir_all(&p).map_err(|e| format!("Failed to create directory: {e}"))?;
    crate::index::tree::invalidate();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn file_exists(path: &str, vault_path_state: tauri::State<'_, VaultPathState>) -> bool {
    ensure_in_vault(path, &vault_path_state)
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// Copy one file to `to`, refusing to replace anything already there. Copies
/// never get the case-only-rename allowance: a copy onto another spelling of its
/// own source would truncate the source it is reading from.
fn copy_file_at(from: &Path, to: &Path) -> Result<(), String> {
    if fs::symlink_metadata(to).is_ok() {
        return Err(occupied_error(to));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    fs::copy(from, to).map_err(|e| format!("Failed to copy file: {e}"))?;
    Ok(())
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
    copy_file_at(&from_path, &to_path)?;
    crate::index::tree::invalidate();
    crate::index::upsert_path(&vault_root(&vault_path_state), &to_path);
    Ok(())
}

/// Copy a file from an arbitrary source **outside** the vault into a
/// vault-contained destination. Used by drag-drop / paste import flows where
/// the user explicitly brings an external file (e.g. an image on the Desktop)
/// into a note as an attachment. Only the *destination* is containment-checked;
/// the source is user-chosen and may live anywhere — mirroring `save_file_bytes`,
/// which writes to a user-picked path outside the vault.
#[tauri::command]
#[specta::specta]
pub fn import_external_file(
    from: &str,
    to: &str,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let to_path = ensure_in_vault(to, &vault_path_state)?;
    copy_file_at(Path::new(from), &to_path)?;
    crate::index::tree::invalidate();
    crate::index::upsert_path(&vault_root(&vault_path_state), &to_path);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn copy_directory(
    from: String,
    to: String,
    vault_path_state: tauri::State<'_, VaultPathState>,
) -> Result<(), String> {
    let src = ensure_in_vault(&from, &vault_path_state)?;
    let dst = ensure_in_vault(&to, &vault_path_state)?;
    // Blocking recursive I/O; paths are resolved first since State cannot cross threads.
    tokio::task::spawn_blocking(move || copy_dir_recursive(&src, &dst))
        .await
        .map_err(|e| e.to_string())??;
    // A directory copy adds an unknown number of notes; the watcher-driven
    // rebuild picks them up, and it skips every file it has already indexed.
    crate::index::tree::invalidate();
    Ok(())
}

/// Copy a directory tree. Walks with [`walk_dir_capped`] so symlinks are never
/// followed and the depth cap applies — a symlinked subdirectory can neither
/// escape the vault nor recurse without bound.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    // Refuse an existing destination: merging a copy into it would interleave
    // the two trees and overwrite whatever shares a name.
    if fs::symlink_metadata(dst).is_ok() {
        return Err(occupied_error(dst));
    }
    fs::create_dir_all(dst).map_err(|e| format!("Failed to create directory: {e}"))?;

    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut error: Option<String> = None;

    walk_dir_capped(src, 0, MAX_WALK_DEPTH, &mut |item| {
        if error.is_some() || item.is_symlink {
            return WalkAction::Skip;
        }
        let Ok(rel) = item.path.strip_prefix(src) else {
            error = Some("Failed to resolve copy destination".into());
            return WalkAction::Skip;
        };
        if item.is_dir {
            if let Err(e) = fs::create_dir_all(dst.join(rel)) {
                error = Some(format!("Failed to create directory: {e}"));
            }
            WalkAction::Recurse
        } else {
            files.push((item.path.clone(), dst.join(rel)));
            WalkAction::Skip
        }
    });
    if let Some(e) = error {
        return Err(e);
    }

    files.par_iter().try_for_each(|(s, d)| {
        fs::copy(s, d)
            .map(|_| ())
            .map_err(|e| format!("Failed to copy file: {e}"))
    })
}

/// Set the modification time of a file to a specific unix timestamp (seconds).
///
/// `mtime` is a u32 (unix seconds) because specta forbids exporting u64 across
/// the IPC boundary. Unix-second timestamps fit in u32 until 2106; the value is
/// widened to i64 for `filetime` below.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "margin-fs-test-{}-{tag}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rename_refuses_an_existing_destination() {
        let dir = temp_dir("rename-conflict");
        let from = dir.join("from.md");
        let to = dir.join("to.md");
        fs::write(&from, "source").unwrap();
        fs::write(&to, "target").unwrap();

        let err = rename_entry_at(&from, &to).unwrap_err();
        assert!(err.contains("to.md"), "error must name the target: {err}");
        assert_eq!(fs::read_to_string(&to).unwrap(), "target");
        assert_eq!(fs::read_to_string(&from).unwrap(), "source");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_into_a_free_path_still_renames() {
        let dir = temp_dir("rename-free");
        let from = dir.join("from.md");
        let to = dir.join("nested").join("to.md");
        fs::write(&from, "source").unwrap();

        rename_entry_at(&from, &to).unwrap();

        assert!(!from.exists());
        assert_eq!(fs::read_to_string(&to).unwrap(), "source");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn case_only_rename_of_the_same_file_changes_its_spelling() {
        let dir = temp_dir("rename-case");
        let lower = dir.join("note.md");
        fs::write(&lower, "body").unwrap();
        // On a case-sensitive filesystem `Note.md` is a different, absent file,
        // so there is no same-entry rename to exercise.
        if !dir.join("Note.md").exists() {
            fs::remove_dir_all(&dir).ok();
            return;
        }
        let vault = VaultPathState(Mutex::new(path_to_string(dir.clone())));
        let from = ensure_in_vault(&path_to_string(lower), &vault).unwrap();
        let to = ensure_in_vault_keep_name(&path_to_string(dir.join("Note.md")), &vault).unwrap();

        rename_entry_at(&from, &to).unwrap();

        let names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Note.md"]);
        assert_eq!(fs::read_to_string(dir.join("Note.md")).unwrap(), "body");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn case_varied_destination_is_a_conflict_when_both_spellings_exist() {
        let dir = temp_dir("rename-case-conflict");
        let lower = dir.join("note.md");
        let upper = dir.join("Note.md");
        fs::write(&lower, "lower").unwrap();
        fs::write(&upper, "upper").unwrap();
        // A case-insensitive filesystem just wrote the same file twice.
        if fs::read_to_string(&lower).unwrap() == "upper" {
            fs::remove_dir_all(&dir).ok();
            return;
        }

        assert!(rename_entry_at(&lower, &upper).is_err());
        assert_eq!(fs::read_to_string(&upper).unwrap(), "upper");
        assert_eq!(fs::read_to_string(&lower).unwrap(), "lower");

        fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_still_blocks_a_rename() {
        let dir = temp_dir("rename-symlink");
        let from = dir.join("from.md");
        fs::write(&from, "source").unwrap();
        let to = dir.join("to.md");
        std::os::unix::fs::symlink(dir.join("missing.md"), &to).unwrap();

        assert!(rename_entry_at(&from, &to).is_err());
        assert_eq!(fs::read_to_string(&from).unwrap(), "source");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copy_and_import_refuse_an_existing_destination() {
        let dir = temp_dir("copy-conflict");
        let src = dir.join("inside.md");
        fs::write(&src, "source").unwrap();
        let outside = dir.join("outside.png");
        fs::write(&outside, "imported").unwrap();

        let existing = dir.join("target.md");
        fs::write(&existing, "target").unwrap();
        let err = copy_file_at(&src, &existing).unwrap_err();
        assert!(
            err.contains("target.md"),
            "error must name the target: {err}"
        );
        assert_eq!(fs::read_to_string(&existing).unwrap(), "target");

        let imported = dir.join("existing.png");
        fs::write(&imported, "keep").unwrap();
        let err = copy_file_at(&outside, &imported).unwrap_err();
        assert!(
            err.contains("existing.png"),
            "error must name the target: {err}"
        );
        assert_eq!(fs::read_to_string(&imported).unwrap(), "keep");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copy_into_a_free_path_still_copies() {
        let dir = temp_dir("copy-free");
        let src = dir.join("inside.md");
        fs::write(&src, "source").unwrap();
        let dst = dir.join("nested").join("copy.md");

        copy_file_at(&src, &dst).unwrap();

        assert_eq!(fs::read_to_string(&dst).unwrap(), "source");
        assert_eq!(fs::read_to_string(&src).unwrap(), "source");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copying_a_directory_onto_an_existing_one_is_refused() {
        let dir = temp_dir("copy-dir");
        let src = dir.join("src");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("sub").join("a.md"), "a").unwrap();
        let occupied = dir.join("occupied");
        fs::create_dir_all(&occupied).unwrap();
        fs::write(occupied.join("keep.md"), "keep").unwrap();

        assert!(copy_dir_recursive(&src, &occupied).is_err());
        assert_eq!(
            fs::read_to_string(occupied.join("keep.md")).unwrap(),
            "keep"
        );

        let free = dir.join("free");
        copy_dir_recursive(&src, &free).unwrap();
        assert_eq!(
            fs::read_to_string(free.join("sub").join("a.md")).unwrap(),
            "a"
        );

        fs::remove_dir_all(&dir).ok();
    }
}
