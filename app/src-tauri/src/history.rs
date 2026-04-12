use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct Snapshot {
    /// Filename of the snapshot (e.g. "1712928000.md")
    pub filename: String,
    /// Unix timestamp (seconds) when the snapshot was taken
    pub timestamp: u64,
    /// Size of the snapshot in bytes
    pub size: u64,
}

/// Build the history directory path for a given file.
/// e.g. vault/.margin/history/notes/foo.md/
fn history_dir(vault_path: &str, file_path: &str) -> Result<String, String> {
    let vault = vault_path.trim_end_matches('/');
    let file = file_path.trim_end_matches('/');

    // file_path must be inside vault_path (check with trailing slash to prevent prefix confusion)
    let vault_prefix = format!("{vault}/");
    if !file.starts_with(&vault_prefix) {
        return Err("File is not inside the vault".into());
    }

    let rel = &file[vault.len()..].trim_start_matches('/');
    let history = format!("{vault}/.margin/history/{rel}");
    Ok(history)
}

/// Maximum number of snapshots kept per file. Oldest are pruned on save.
const MAX_SNAPSHOTS_PER_FILE: usize = 50;

/// Save a snapshot of the given file content.
/// The snapshot is stored as `<timestamp>.md` inside the history directory.
/// If the number of snapshots exceeds the limit, the oldest are pruned.
#[tauri::command]
pub fn save_snapshot(
    vault_path: &str,
    file_path: &str,
    content: Vec<u8>,
) -> Result<String, String> {
    let dir = history_dir(vault_path, file_path)?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create history dir: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Use the same extension as the original file
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("md");

    let filename = format!("{timestamp}.{ext}");
    let snapshot_path = format!("{dir}/{filename}");
    let snapshot_tmp = format!("{dir}/.{filename}.tmp");

    // Write to a temp file first; rename atomically into place so a crash
    // mid-write doesn't leave a partial (unreadable) snapshot.
    fs::write(&snapshot_tmp, &content).map_err(|e| format!("Failed to write snapshot: {e}"))?;
    fs::rename(&snapshot_tmp, &snapshot_path).map_err(|e| {
        let _ = fs::remove_file(&snapshot_tmp);
        format!("Failed to finalise snapshot: {e}")
    })?;

    // Prune oldest snapshots if over the limit
    prune_old_snapshots(&dir, MAX_SNAPSHOTS_PER_FILE);

    Ok(filename)
}

/// Keep at most `max_count` snapshots in a directory, deleting the oldest.
fn prune_old_snapshots(dir: &str, max_count: usize) {
    let dir_path = Path::new(dir);
    let entries = match fs::read_dir(dir_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut files: Vec<(u64, std::path::PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let ts: u64 = name.split('.').next()?.parse().ok()?;
            Some((ts, path))
        })
        .collect();

    if files.len() <= max_count {
        return;
    }

    // Sort oldest first
    files.sort_by_key(|(ts, _)| *ts);
    let to_remove = files.len() - max_count;
    for (_, path) in files.into_iter().take(to_remove) {
        let _ = fs::remove_file(path);
    }
}

/// List all snapshots for a given file, sorted newest-first.
#[tauri::command]
pub fn list_snapshots(vault_path: &str, file_path: &str) -> Result<Vec<Snapshot>, String> {
    let dir = history_dir(vault_path, file_path)?;
    let dir_path = Path::new(&dir);

    if !dir_path.is_dir() {
        return Ok(vec![]);
    }

    let entries = fs::read_dir(dir_path).map_err(|e| format!("Failed to read history dir: {e}"))?;

    let mut snapshots: Vec<Snapshot> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            // Parse timestamp from filename: "<timestamp>.<ext>"
            let ts_str = name.split('.').next()?;
            let timestamp: u64 = ts_str.parse().ok()?;
            let size = entry.metadata().ok()?.len();
            Some(Snapshot {
                filename: name,
                timestamp,
                size,
            })
        })
        .collect();

    // Newest first
    snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(snapshots)
}

/// Resolve a snapshot path and verify it stays within the history directory (prevents symlink attacks).
fn safe_snapshot_path(
    vault_path: &str,
    file_path: &str,
    snapshot_filename: &str,
) -> Result<String, String> {
    // Basic character validation
    if snapshot_filename.contains('/')
        || snapshot_filename.contains('\\')
        || snapshot_filename.contains("..")
    {
        return Err("Invalid snapshot filename".into());
    }

    let dir = history_dir(vault_path, file_path)?;
    let snapshot_path = format!("{dir}/{snapshot_filename}");
    let snap = Path::new(&snapshot_path);

    // If the file exists, canonicalize to resolve symlinks and verify containment
    if snap.exists() {
        let canonical = snap
            .canonicalize()
            .map_err(|e| format!("Failed to resolve snapshot path: {e}"))?;
        let dir_canonical = Path::new(&dir)
            .canonicalize()
            .map_err(|e| format!("Failed to resolve history dir: {e}"))?;
        if !canonical.starts_with(&dir_canonical) {
            return Err("Snapshot path escapes history directory".into());
        }
    }

    Ok(snapshot_path)
}

/// Read the content of a specific snapshot.
#[tauri::command]
pub fn read_snapshot(
    vault_path: &str,
    file_path: &str,
    snapshot_filename: &str,
) -> Result<Vec<u8>, String> {
    let snapshot_path = safe_snapshot_path(vault_path, file_path, snapshot_filename)?;
    fs::read(&snapshot_path).map_err(|e| format!("Failed to read snapshot: {e}"))
}

/// Delete a specific snapshot.
#[tauri::command]
pub fn delete_snapshot(
    vault_path: &str,
    file_path: &str,
    snapshot_filename: &str,
) -> Result<(), String> {
    let snapshot_path = safe_snapshot_path(vault_path, file_path, snapshot_filename)?;
    fs::remove_file(&snapshot_path).map_err(|e| format!("Failed to delete snapshot: {e}"))
}

/// Delete all snapshots for a given file.
#[tauri::command]
pub fn clear_snapshots(vault_path: &str, file_path: &str) -> Result<u64, String> {
    let dir = history_dir(vault_path, file_path)?;
    let dir_path = Path::new(&dir);

    if !dir_path.is_dir() {
        return Ok(0);
    }

    let entries = fs::read_dir(dir_path).map_err(|e| format!("Failed to read history dir: {e}"))?;

    let mut count: u64 = 0;
    for entry in entries.flatten() {
        if entry.path().is_file() {
            fs::remove_file(entry.path()).map_err(|e| format!("Failed to delete snapshot: {e}"))?;
            count += 1;
        }
    }

    // Remove the directory itself if empty
    let _ = fs::remove_dir(dir_path);

    Ok(count)
}

/// Delete the entire history subtree for a given path (file or directory).
/// When a directory is deleted, all history for every file beneath it is removed.
#[tauri::command]
pub fn clear_history_tree(vault_path: &str, entry_path: &str) -> Result<(), String> {
    let vault = vault_path.trim_end_matches('/');
    let entry = entry_path.trim_end_matches('/');

    let vault_prefix = format!("{vault}/");
    if !entry.starts_with(&vault_prefix) {
        return Err("Path is not inside the vault".into());
    }

    let rel = &entry[vault.len()..].trim_start_matches('/');
    let history_path = format!("{vault}/.margin/history/{rel}");
    let p = Path::new(&history_path);

    if p.exists() {
        if p.is_dir() {
            fs::remove_dir_all(p).map_err(|e| format!("Failed to remove history tree: {e}"))?;
        } else {
            fs::remove_file(p).map_err(|e| format!("Failed to remove history entry: {e}"))?;
        }
    }

    Ok(())
}

/// Move/rename the history directory when a file or directory is renamed.
#[tauri::command]
pub fn rename_history(vault_path: &str, old_path: &str, new_path: &str) -> Result<(), String> {
    let vault = vault_path.trim_end_matches('/');
    let old = old_path.trim_end_matches('/');
    let new_ = new_path.trim_end_matches('/');

    let vault_prefix = format!("{vault}/");
    if !old.starts_with(&vault_prefix) || !new_.starts_with(&vault_prefix) {
        return Err("Paths must be inside the vault".into());
    }

    let old_rel = &old[vault.len()..].trim_start_matches('/');
    let new_rel = &new_[vault.len()..].trim_start_matches('/');

    let old_history = format!("{vault}/.margin/history/{old_rel}");
    let new_history = format!("{vault}/.margin/history/{new_rel}");

    let old_p = Path::new(&old_history);
    if !old_p.exists() {
        return Ok(()); // No history to move
    }

    // Ensure parent of destination exists
    let new_p = Path::new(&new_history);
    if let Some(parent) = new_p.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create history parent dir: {e}"))?;
    }

    fs::rename(&old_history, &new_history).map_err(|e| format!("Failed to rename history: {e}"))?;

    Ok(())
}
