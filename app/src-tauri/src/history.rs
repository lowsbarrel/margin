use crate::fs::{normalise_slashes, rel_under_vault};
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

mod retention;

#[cfg(test)]
mod tests;

use retention::{now_millis, prune_snapshots, snapshot_files, snapshot_millis};

// A `..` component would escape .margin/history through the string concatenation in `history_dir`.
fn reject_parent_dir(rel: &str) -> Result<(), String> {
    if Path::new(rel)
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("Path must not contain '..'".into());
    }
    Ok(())
}

#[derive(Serialize, specta::Type)]
pub struct Snapshot {
    pub filename: String,
    #[specta(type = u32)]
    pub timestamp: u64,
    #[specta(type = u32)]
    pub size: u64,
}

fn history_dir(vault_path: &str, file_path: &str) -> Result<String, String> {
    let vault = normalise_slashes(vault_path);
    let file = normalise_slashes(file_path);
    let rel =
        rel_under_vault(&vault, &file).ok_or_else(|| "File is not inside the vault".to_string())?;
    reject_parent_dir(&rel)?;
    let vault = vault.trim_end_matches('/');
    Ok(format!("{vault}/.margin/history/{rel}"))
}

pub(crate) fn save_snapshot_inner(
    vault_path: &str,
    file_path: &str,
    content: &[u8],
) -> Result<String, String> {
    let dir = history_dir(vault_path, file_path)?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create history dir: {e}"))?;

    let existing = snapshot_files(Path::new(&dir));
    if let Some((_, newest)) = existing.iter().max_by_key(|(ts, _)| *ts)
        && let Ok(bytes) = fs::read(Path::new(&dir).join(newest))
        && bytes == content
    {
        prune_snapshots(Path::new(&dir));
        return Ok(newest.clone());
    }

    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("md");

    let mut timestamp = now_millis();
    let mut filename = format!("{timestamp}.{ext}");
    while Path::new(&dir).join(&filename).exists() {
        timestamp += 1;
        filename = format!("{timestamp}.{ext}");
    }

    crate::fs::atomic_write(&Path::new(&dir).join(&filename), content)?;
    prune_snapshots(Path::new(&dir));

    Ok(filename)
}

#[tauri::command]
#[specta::specta]
pub fn save_snapshot(
    vault_path: &str,
    file_path: &str,
    content: Vec<u8>,
) -> Result<String, String> {
    save_snapshot_inner(vault_path, file_path, &content)
}

#[tauri::command]
#[specta::specta]
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
            let timestamp = snapshot_millis(&name)? / 1000;
            let size = entry.metadata().ok()?.len();
            Some(Snapshot {
                filename: name,
                timestamp,
                size,
            })
        })
        .collect();

    snapshots.sort_by_key(|s| std::cmp::Reverse(s.timestamp));

    Ok(snapshots)
}

// The name check alone would accept a symlink pointing outside the history dir, so an existing snapshot is resolved and contained.
fn safe_snapshot_path(
    vault_path: &str,
    file_path: &str,
    snapshot_filename: &str,
) -> Result<String, String> {
    if snapshot_filename.contains('/')
        || snapshot_filename.contains('\\')
        || snapshot_filename.contains("..")
    {
        return Err("Invalid snapshot filename".into());
    }

    let dir = history_dir(vault_path, file_path)?;
    let snapshot_path = format!("{dir}/{snapshot_filename}");
    let snap = Path::new(&snapshot_path);

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

#[tauri::command]
#[specta::specta]
pub fn read_snapshot(
    vault_path: &str,
    file_path: &str,
    snapshot_filename: &str,
) -> Result<Vec<u8>, String> {
    let snapshot_path = safe_snapshot_path(vault_path, file_path, snapshot_filename)?;
    fs::read(&snapshot_path).map_err(|e| format!("Failed to read snapshot: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn delete_snapshot(
    vault_path: &str,
    file_path: &str,
    snapshot_filename: &str,
) -> Result<(), String> {
    let snapshot_path = safe_snapshot_path(vault_path, file_path, snapshot_filename)?;
    fs::remove_file(&snapshot_path).map_err(|e| format!("Failed to delete snapshot: {e}"))
}

#[tauri::command]
#[specta::specta]
pub fn clear_snapshots(vault_path: &str, file_path: &str) -> Result<u32, String> {
    let dir = history_dir(vault_path, file_path)?;
    let dir_path = Path::new(&dir);

    if !dir_path.is_dir() {
        return Ok(0);
    }

    let entries = fs::read_dir(dir_path).map_err(|e| format!("Failed to read history dir: {e}"))?;

    let mut count: u32 = 0;
    for entry in entries.flatten() {
        if entry.path().is_file() {
            fs::remove_file(entry.path()).map_err(|e| format!("Failed to delete snapshot: {e}"))?;
            count += 1;
        }
    }

    let _ = fs::remove_dir(dir_path);

    Ok(count)
}

#[tauri::command]
#[specta::specta]
pub fn rename_history(vault_path: &str, old_path: &str, new_path: &str) -> Result<(), String> {
    let vault = normalise_slashes(vault_path);
    let old = normalise_slashes(old_path);
    let new_ = normalise_slashes(new_path);

    let old_rel = rel_under_vault(&vault, &old)
        .ok_or_else(|| "Paths must be inside the vault".to_string())?;
    let new_rel = rel_under_vault(&vault, &new_)
        .ok_or_else(|| "Paths must be inside the vault".to_string())?;
    reject_parent_dir(&old_rel)?;
    reject_parent_dir(&new_rel)?;

    let vault = vault.trim_end_matches('/');
    let old_history = format!("{vault}/.margin/history/{old_rel}");
    let new_history = format!("{vault}/.margin/history/{new_rel}");

    let old_p = Path::new(&old_history);
    if !old_p.exists() {
        return Ok(());
    }

    merge_history_dirs(old_p, Path::new(&new_history))
}

pub(crate) fn merge_history_dirs(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("Failed to create history parent dir: {e}"))?;

    let entries = fs::read_dir(src).map_err(|e| format!("Failed to read history dir: {e}"))?;
    for entry in entries.flatten() {
        let target = dst.join(entry.file_name());
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            merge_history_dirs(&entry.path(), &target)?;
            continue;
        }
        let source = entry.path();
        if let Ok(existing) = fs::read(&target)
            && existing == fs::read(&source).unwrap_or_default()
        {
            let _ = fs::remove_file(&source);
            continue;
        }
        let target = if target.exists() {
            free_snapshot_path(dst, &entry.file_name().to_string_lossy())?
        } else {
            target
        };
        fs::rename(&source, &target).map_err(|e| format!("Failed to move snapshot: {e}"))?;
    }

    let _ = fs::remove_dir(src);
    Ok(())
}

fn free_snapshot_path(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    if let Ok(mut timestamp) = stem.parse::<u64>() {
        loop {
            timestamp += 1;
            let candidate = dir.join(format!("{timestamp}{ext}"));
            if !candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    for n in 2..1000 {
        let candidate = dir.join(format!("{stem}-{n}{ext}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(format!("No free snapshot name for {name}"))
}

pub(crate) fn prune_orphans(vault_path: &str) -> u32 {
    let history_root = Path::new(vault_path).join(".margin/history");
    if !history_root.is_dir() {
        return 0;
    }
    let vault_root = Path::new(vault_path);
    let mut removed = 0;
    prune_orphan_dir(&history_root, vault_root, &mut removed);
    removed
}

fn prune_orphan_dir(dir: &Path, vault_file: &Path, removed: &mut u32) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    let mut kept_children = 0;
    let mut kept_files = 0;
    for entry in entries.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            if prune_orphan_dir(&entry.path(), &vault_file.join(entry.file_name()), removed) {
                kept_children += 1;
            }
        } else {
            kept_files += 1;
        }
    }

    let keep = kept_children > 0 || (kept_files > 0 && vault_file.is_file());
    if !keep && fs::remove_dir_all(dir).is_ok() {
        *removed += 1;
    }
    keep
}
