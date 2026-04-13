use notify::{recommended_watcher, Event, EventKind, RecursiveMode, Watcher};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Emitter, Manager};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Convert a `PathBuf` / `OsString` into a `String` with forward slashes
/// so the JS frontend gets consistent separators on every platform.
#[inline]
fn path_to_string(p: std::path::PathBuf) -> String {
    let s = p.into_os_string().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
    #[cfg(target_os = "windows")]
    { s.replace('\\', "/") }
    #[cfg(not(target_os = "windows"))]
    { s }
}

/// Write `content` to `dest` atomically by writing to a sibling temp file
/// and then renaming it into place. Prevents partial writes from corrupting
/// the target file if the process or OS crashes mid-write.
fn atomic_write(dest: &Path, content: &[u8]) -> Result<(), String> {
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

#[derive(Serialize)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub path: String,
    /// Seconds since UNIX epoch (file modification time). 0 if unavailable.
    pub modified: u64,
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
    // Ensure .margin/docs/ exists
    let margin_dir = p.join(".margin").join("docs");
    fs::create_dir_all(&margin_dir).map_err(|e| format!("Failed to create .margin/docs: {e}"))?;
    // Store the vault path so the localfile:// handler can enforce containment.
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
            // Fallback: deserialize JSON number array (for backwards compat)
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

/// Recursively walk an entire directory tree in a single call, returning all
/// entries (files and directories). Hidden entries (starting with `.`) are
/// skipped unless `include_hidden` is true. This replaces the pattern of
/// making one IPC call per directory from JS — at 200 directories the savings
/// are ~200x on round-trip overhead.
#[tauri::command]
pub fn walk_directory(root: &str, include_hidden: bool) -> Result<Vec<FsEntry>, String> {
    let mut entries = Vec::new();
    walk_dir_impl(Path::new(root), include_hidden, &mut entries);
    Ok(entries)
}

fn walk_dir_impl(dir: &Path, include_hidden: bool, result: &mut Vec<FsEntry>) {
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in read.flatten() {
        let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
        if !include_hidden && name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path_str = path_to_string(entry.path());
        if is_dir {
            let child_path = Path::new(&path_str).to_path_buf();
            result.push(FsEntry {
                name,
                is_dir: true,
                path: path_str,
                modified,
            });
            walk_dir_impl(&child_path, include_hidden, result);
        } else {
            result.push(FsEntry {
                name,
                is_dir: false,
                path: path_str,
                modified,
            });
        }
    }
}

/// A single row in the file tree, pre-sorted and depth-annotated.
/// The frontend renders this as a flat virtual-scroll list instead of
/// a recursive component tree with one IPC call per expanded folder.
#[derive(Serialize, Clone)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub modified: u64,
    /// Nesting depth (0 = vault root level).
    pub depth: usize,
}

/// Build a flat, sorted, depth-annotated list of every currently-visible
/// tree row in a single native call.
///
/// Directories listed in `expanded` are inlined at their position with their
/// children directly beneath them (sorted). Hidden entries (`.` prefix) are
/// skipped. Sorting is stable within each level: dirs first, then either
/// alphabetical (`sort_by = "name"`) or modification time descending
/// (`sort_by = "date"`).
///
/// This replaces the pattern of one `list_directory` IPC call per expanded
/// folder. At 200 open folders the old approach made 200 IPC round-trips;
/// this is always 1.
#[tauri::command]
pub fn build_visible_tree(
    root: &str,
    expanded: Vec<String>,
    sort_by: &str,
) -> Result<Vec<TreeEntry>, String> {
    let expanded_set: HashSet<String> = expanded.into_iter().collect();
    let mut results = Vec::new();
    build_tree_impl(Path::new(root), 0, &expanded_set, sort_by, &mut results);
    Ok(results)
}

fn build_tree_impl(
    dir: &Path,
    depth: usize,
    expanded: &HashSet<String>,
    sort_by: &str,
    result: &mut Vec<TreeEntry>,
) {
    struct Raw {
        name: String,
        path: std::path::PathBuf,
        is_dir: bool,
        modified: u64,
    }

    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut entries: Vec<Raw> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
            if name.starts_with('.') {
                return None;
            }
            let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let modified = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            Some(Raw {
                name,
                path: e.path(),
                is_dir,
                modified,
            })
        })
        .collect();

    // Sort: directories always before files, then within each group by sort_by.
    if sort_by == "date" {
        entries.sort_unstable_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            (false, false) => b.modified.cmp(&a.modified),
        });
    } else {
        entries.sort_unstable_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
    }

    for raw in entries {
        let path_str = path_to_string(raw.path.clone());
        let is_dir = raw.is_dir;
        result.push(TreeEntry {
            name: raw.name,
            path: path_str.clone(),
            is_dir,
            modified: raw.modified,
            depth,
        });
        if is_dir && expanded.contains(&path_str) {
            build_tree_impl(Path::new(&path_str), depth + 1, expanded, sort_by, result);
        }
    }
}

/// Returned by `read_link_batch`.
#[derive(Serialize)]
pub struct LinkEntry {
    pub path: String,
    pub links: Vec<String>,
}

/// Read multiple Markdown files and extract `[[wiki-links]]` from each — all
/// in native Rust using a single IPC call. This replaces the JS pattern of
/// looping through hundreds or thousands of files with individual IPC reads.
///
/// At 100k files, the JS approach needed ~3,125 round-trips (batches of 32).
/// This command reduces that to 1 round-trip regardless of vault size.
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
/// Uses a simple byte scanner — faster than pulling in the `regex` crate,
/// and sufficient for the `[[...]]` pattern.
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
            // Exclude image embeds preceded by `!`
            if i > 0 && bytes[i - 1] == b'!' {
                i += 2;
                continue;
            }
            i += 2;
            let start = i;
            // Scan forward until `]]` or end of line
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
            continue; // skip hidden files/dirs
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

    // Collect all entries first so we can separate dirs (must be sequential)
    // from files (can be parallel).
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

    // Copy files in parallel
    files_to_copy
        .par_iter()
        .try_for_each(|(s, d)| {
            fs::copy(s, d)
                .map(|_| ())
                .map_err(|e| format!("Failed to copy file: {e}"))
        })?;

    // Recurse into subdirectories
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
    let status = if target.is_dir() {
        Command::new("explorer").arg(target).status()
    } else {
        Command::new("explorer")
            .arg(format!("/select,{}", target.display()))
            .status()
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

    let status = status.map_err(|e| format!("Failed to open file manager: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("File manager exited with status {status}"))
    }
}

#[tauri::command]
pub fn watch_file(app: AppHandle, path: String) -> Result<(), String> {
    let watcher_state = app.state::<WatcherState>();
    let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;

    // Stop existing watcher
    *guard = None;

    let watch_path = path.clone();
    let app_handle = app.clone();

    let mut watcher = recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    let _ = app_handle.emit("file-changed", &watch_path);
                }
                _ => {}
            }
        }
    })
    .map_err(|e| format!("Failed to create watcher: {e}"))?;

    watcher
        .watch(Path::new(&path), RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch file: {e}"))?;

    *guard = Some(watcher);
    Ok(())
}

#[tauri::command]
pub fn unwatch_file(app: AppHandle) -> Result<(), String> {
    let watcher_state = app.state::<WatcherState>();
    let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}

/// Watch the entire vault directory recursively. Emits `"vault-fs-changed"`
/// immediately whenever a non-hidden file is created, modified or deleted.
/// On macOS this uses FSEvents which is O(1) kernel overhead regardless of
/// file count.
#[tauri::command]
pub fn watch_vault(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<VaultWatcherState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;

    // Stop any previous vault watcher
    *guard = None;

    let vault_root = Path::new(&path).to_path_buf();
    let app_handle = app.clone();

    let mut watcher = recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    // Skip hidden files/folders (. prefix) — includes .margin/, .git/, etc.
                    let all_hidden = event.paths.iter().all(|p| {
                        p.strip_prefix(&vault_root)
                            .map(|rel| {
                                rel.components().any(|c| {
                                    c.as_os_str()
                                        .to_str()
                                        .map(|s| s.starts_with('.'))
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false)
                    });
                    if all_hidden {
                        return;
                    }
                    let _ = app_handle.emit("vault-fs-changed", ());
                }
                _ => {}
            }
        }
    })
    .map_err(|e| format!("Failed to create vault watcher: {e}"))?;

    watcher
        .watch(Path::new(&path), RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch vault: {e}"))?;

    *guard = Some(watcher);
    Ok(())
}

#[tauri::command]
pub fn unwatch_vault(app: AppHandle) -> Result<(), String> {
    let state = app.state::<VaultWatcherState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}

#[tauri::command]
pub fn search_files(root: &str, query: &str) -> Result<Vec<FsEntry>, String> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    const MAX_RESULTS: usize = 200;
    const MAX_DEPTH: usize = 10;
    collect_files_recursive(
        Path::new(root),
        &query_lower,
        &mut results,
        0,
        MAX_DEPTH,
        MAX_RESULTS,
    );
    // Sort: exact prefix matches first, then alphabetical
    results.sort_by(|a, b| {
        let a_starts = a.name.to_lowercase().starts_with(&query_lower);
        let b_starts = b.name.to_lowercase().starts_with(&query_lower);
        match (a_starts, b_starts) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    Ok(results)
}

fn collect_files_recursive(
    dir: &Path,
    query: &str,
    results: &mut Vec<FsEntry>,
    depth: usize,
    max_depth: usize,
    max_results: usize,
) {
    if depth >= max_depth || results.len() >= max_results {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if results.len() >= max_results {
            return;
        }
        let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            collect_files_recursive(&path, query, results, depth + 1, max_depth, max_results);
        } else if name.to_lowercase().contains(query) {
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            results.push(FsEntry {
                name,
                is_dir: false,
                path: path_to_string(path),
                modified,
            });
        }
    }
}

#[derive(Serialize)]
pub struct ContentMatch {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub context: String,
}

#[tauri::command]
pub fn search_file_contents(
    root: &str,
    query: &str,
    case_sensitive: bool,
) -> Result<Vec<ContentMatch>, String> {
    if query.is_empty() {
        return Ok(vec![]);
    }
    const MAX_RESULTS: usize = 500;
    const MAX_DEPTH: usize = 10;
    const MAX_FILES: usize = 5000;

    // Phase 1: collect .md paths (lightweight, single-thread)
    let mut md_paths = Vec::new();
    collect_md_paths(Path::new(root), &mut md_paths, 0, MAX_DEPTH);
    md_paths.truncate(MAX_FILES);

    // Phase 2: pre-compute query once
    let query_cmp = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    // Phase 3: search in parallel, each file produces its own matches
    let all_matches: Vec<Vec<ContentMatch>> = md_paths
        .into_par_iter()
        .map(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => return vec![],
            };
            let path_str = path_to_string(path);
            let mut matches = Vec::new();

            if case_sensitive {
                // No allocations — search the original lines directly
                for (line_idx, line) in content.lines().enumerate() {
                    let mut search_from = 0;
                    while let Some(col) = line[search_from..].find(&*query_cmp) {
                        let actual_col = search_from + col;
                        matches.push(build_content_match(
                            &path_str,
                            &name,
                            line,
                            line_idx,
                            actual_col,
                            query.len(),
                        ));
                        search_from = actual_col + query_cmp.len();
                    }
                }
            } else {
                // Lowercase the entire content once, then search. The line
                // offsets into content_lower correspond to the same byte
                // positions in content (to_lowercase() preserves byte count
                // for ASCII; for non-ASCII this could be off, but markdown
                // files are overwhelmingly ASCII).
                let content_lower = content.to_lowercase();
                let lines_lower: Vec<&str> = content_lower.lines().collect();
                let lines_orig: Vec<&str> = content.lines().collect();
                for line_idx in 0..lines_lower.len() {
                    let line_l = lines_lower[line_idx];
                    let line_o = if line_idx < lines_orig.len() {
                        lines_orig[line_idx]
                    } else {
                        line_l
                    };
                    let mut search_from = 0;
                    while let Some(col) = line_l[search_from..].find(&*query_cmp) {
                        let actual_col = search_from + col;
                        matches.push(build_content_match(
                            &path_str,
                            &name,
                            line_o,
                            line_idx,
                            actual_col,
                            query.len(),
                        ));
                        search_from = actual_col + query_cmp.len();
                    }
                }
            }
            matches
        })
        .collect();

    // Phase 4: flatten & truncate
    let mut results: Vec<ContentMatch> = all_matches.into_iter().flatten().collect();
    results.truncate(MAX_RESULTS);
    Ok(results)
}

/// Build a ContentMatch with surrounding context, snapping to char boundaries.
fn build_content_match(
    path: &str,
    name: &str,
    line: &str,
    line_idx: usize,
    col: usize,
    query_len: usize,
) -> ContentMatch {
    let mut ctx_start = col.saturating_sub(30);
    while ctx_start > 0 && !line.is_char_boundary(ctx_start) {
        ctx_start -= 1;
    }
    let mut ctx_end = std::cmp::min(line.len(), col + query_len + 60);
    while ctx_end < line.len() && !line.is_char_boundary(ctx_end) {
        ctx_end += 1;
    }
    ContentMatch {
        path: path.to_string(),
        name: name.to_string(),
        line: line_idx + 1,
        column: col + 1,
        context: line[ctx_start..ctx_end].to_string(),
    }
}

#[tauri::command]
pub fn replace_in_file(
    path: &str,
    search: &str,
    replace: &str,
    case_sensitive: bool,
) -> Result<usize, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {e}"))?;
    let (new_content, count) = if case_sensitive {
        let count = content.matches(search).count();
        (content.replace(search, replace), count)
    } else {
        let mut result = String::with_capacity(content.len());
        let search_lower = search.to_lowercase();
        let mut last_end = 0;
        let content_lower = content.to_lowercase();
        let mut count = 0usize;
        while let Some(start) = content_lower[last_end..].find(&search_lower) {
            let abs_start = last_end + start;
            result.push_str(&content[last_end..abs_start]);
            result.push_str(replace);
            last_end = abs_start + search.len();
            count += 1;
        }
        result.push_str(&content[last_end..]);
        (result, count)
    };
    if count > 0 {
        atomic_write(Path::new(path), new_content.as_bytes())?;
    }
    Ok(count)
}

// ─── Tag extraction ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TagInfo {
    pub tag: String,
    pub count: usize,
    pub files: Vec<String>,
}

/// Extract `#tag` tokens from markdown content.
/// Skips fenced code blocks and headings.
/// A tag is `#` followed immediately by a letter, at a word boundary.
/// Uses a byte-level scanner — no per-line Vec<char> allocation.
fn extract_tags_from_content(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        let bytes = trimmed.as_bytes();

        // Toggle code block state
        if bytes.starts_with(b"```") || bytes.starts_with(b"~~~") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }

        // Skip markdown headings (# Heading, ## Heading, …)
        if bytes.first() == Some(&b'#') {
            let hash_end = bytes.iter().position(|&b| b != b'#').unwrap_or(bytes.len());
            if hash_end >= bytes.len() || bytes[hash_end] == b' ' {
                continue;
            }
        }

        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'#' {
                // Word-boundary: previous byte must not be alphanumeric, `_`, or `#`
                let prev_ok = i == 0 || {
                    let p = bytes[i - 1];
                    !p.is_ascii_alphanumeric() && p != b'_' && p != b'#'
                };
                // Next byte must be an ASCII letter (tags are ASCII-only identifiers)
                if prev_ok && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
                    let start = i + 1;
                    let mut j = start;
                    while j < bytes.len()
                        && (bytes[j].is_ascii_alphanumeric()
                            || bytes[j] == b'-'
                            || bytes[j] == b'_'
                            || bytes[j] == b'/')
                    {
                        j += 1;
                    }
                    // Safe: we only accepted ASCII bytes
                    let tag = std::str::from_utf8(&bytes[start..j])
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if !tag.is_empty() {
                        tags.push(tag);
                    }
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
    }

    tags
}

/// Scan all `.md` files under `root` and collect unique `#tag` tokens.
/// Returns a list sorted by occurrence count (descending).
/// Uses rayon to parse files across all CPU cores.
#[tauri::command]
pub fn list_all_tags(root: &str) -> Result<Vec<TagInfo>, String> {
    // Phase 1: collect all .md paths with a quick recursive walk (single-threaded, I/O-bound)
    let mut md_paths = Vec::new();
    collect_md_paths(Path::new(root), &mut md_paths, 0, 10);

    // Phase 2: read & extract tags in parallel across cores
    let per_file: Vec<(String, Vec<String>)> = md_paths
        .into_par_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let tags = extract_tags_from_content(&content);
            Some((path_to_string(path), tags))
        })
        .collect();

    // Phase 3: merge into tag->files map (single-threaded, fast)
    let mut tag_files: HashMap<String, Vec<String>> = HashMap::new();
    for (path_str, tags) in per_file {
        let mut seen: HashSet<String> = HashSet::new();
        for tag in tags {
            if seen.insert(tag.clone()) {
                tag_files.entry(tag).or_default().push(path_str.clone());
            }
        }
    }

    let mut result: Vec<TagInfo> = tag_files
        .into_iter()
        .map(|(tag, files)| {
            let count = files.len();
            TagInfo { tag, count, files }
        })
        .collect();

    result.sort_by(|a, b| b.count.cmp(&a.count).then(a.tag.cmp(&b.tag)));
    Ok(result)
}

fn collect_md_paths(dir: &Path, out: &mut Vec<std::path::PathBuf>, depth: usize, max_depth: usize) {
    if depth >= max_depth {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            collect_md_paths(&path, out, depth + 1, max_depth);
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
}

#[tauri::command]
pub fn export_vault_zip(vault_path: &str, dest_path: &str) -> Result<(), String> {
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let root = Path::new(vault_path);
    if !root.is_dir() {
        return Err("Vault path is not a directory".into());
    }

    let file =
        std::fs::File::create(dest_path).map_err(|e| format!("Failed to create zip file: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    fn walk_zip(
        base: &Path,
        dir: &Path,
        zip: &mut ZipWriter<std::fs::File>,
        options: SimpleFileOptions,
    ) -> Result<(), String> {
        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))?;
        for entry in entries.flatten() {
            let name = entry.file_name().into_string().unwrap_or_else(|s| s.to_string_lossy().into_owned());
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .map_err(|e| format!("Path error: {e}"))?
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                zip.add_directory(format!("{relative}/"), options)
                    .map_err(|e| format!("Failed to add directory to zip: {e}"))?;
                walk_zip(base, &path, zip, options)?;
            } else {
                zip.start_file(&relative, options)
                    .map_err(|e| format!("Failed to start file in zip: {e}"))?;
                let mut f =
                    std::fs::File::open(&path).map_err(|e| format!("Failed to open file: {e}"))?;
                std::io::copy(&mut f, zip)
                    .map_err(|e| format!("Failed to write to zip: {e}"))?;
            }
        }
        Ok(())
    }

    walk_zip(root, root, &mut zip, options)?;
    zip.finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;
    Ok(())
}

// ─── Unsynced-changes check (fully Rust-side) ────────────────────────────

/// Manifest entry matching the JSON shape produced by s3sync.ts.
#[derive(Deserialize)]
struct ManifestEntry {
    path: String,
    #[allow(dead_code)]
    hash: String,
    modified: u64,
    deleted_at: Option<u64>,
}

#[derive(Deserialize)]
struct Manifest {
    #[allow(dead_code)]
    version: u64,
    files: Vec<ManifestEntry>,
}

/// Decrypt nonce‖ciphertext with AES-256-GCM-SIV (same scheme as crypto.rs).
fn decrypt_blob_inline(ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm_siv::{aead::Aead, Aes256GcmSiv, KeyInit, Nonce};
    if key.len() != 32 {
        return Err("Key must be 32 bytes".into());
    }
    if ciphertext.len() < 12 {
        return Err("Ciphertext too short".into());
    }
    let cipher =
        Aes256GcmSiv::new_from_slice(key).map_err(|e| format!("Invalid key: {e}"))?;
    let nonce = Nonce::from_slice(&ciphertext[..12]);
    cipher
        .decrypt(nonce, &ciphertext[12..])
        .map_err(|e| format!("Decryption failed: {e}"))
}

/// Collect all non-hidden files in the vault, returning (relative_path, mtime_secs).
/// Skips any path component starting with `.`.
fn walk_vault_files(root: &Path) -> Vec<(String, u64)> {
    let mut result = Vec::new();
    walk_vault_impl(root, root, &mut result);
    result
}

fn walk_vault_impl(base: &Path, dir: &Path, out: &mut Vec<(String, u64)>) {
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            walk_vault_impl(base, &entry.path(), out);
        } else {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if let Ok(rel) = entry.path().strip_prefix(base) {
                out.push((rel.to_string_lossy().into_owned(), mtime));
            }
        }
    }
}

/// Check whether the vault has local changes compared to the last-synced
/// base manifest.  Runs entirely in Rust — no IPC round-trips for
/// individual files.  Compares file existence + mtime only (no hashing),
/// so this is O(n) where n = number of vault files.
#[tauri::command]
pub fn has_unsynced_changes(vault_path: &str, encryption_key: Vec<u8>) -> Result<bool, String> {
    let manifest_path = Path::new(vault_path)
        .join(".margin")
        .join("sync-base.enc");

    // Load & decrypt base manifest
    let manifest: Manifest = if manifest_path.exists() {
        let enc = fs::read(&manifest_path)
            .map_err(|e| format!("Failed to read manifest: {e}"))?;
        let dec = decrypt_blob_inline(&enc, &encryption_key)?;
        serde_json::from_slice(&dec)
            .map_err(|e| format!("Failed to parse manifest: {e}"))?
    } else {
        Manifest {
            version: 2,
            files: Vec::new(),
        }
    };

    // Build map of base entries (exclude tombstones)
    let base: HashMap<&str, u64> = manifest
        .files
        .iter()
        .filter(|e| e.deleted_at.is_none())
        .map(|e| (e.path.as_str(), e.modified))
        .collect();

    // Walk vault
    let local_files = walk_vault_files(Path::new(vault_path));

    // Fast path: different count → definitely changed
    if local_files.len() != base.len() {
        return Ok(true);
    }

    // Check each local file against base
    for (path, mtime) in &local_files {
        match base.get(path.as_str()) {
            None => return Ok(true),        // new file
            Some(&bm) if bm != *mtime => return Ok(true), // changed mtime
            _ => {}
        }
    }

    Ok(false)
}
