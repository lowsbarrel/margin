use crate::fs::{normalise_slashes, rel_under_vault};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Reject a relative path that contains a `..` (parent-dir) component so a
/// caller cannot escape the `.margin/history` subtree via the string-concat
/// path construction below. Empty `rel` is allowed (whole-vault history root).
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
    /// Filename of the snapshot (e.g. "1712928000123.md")
    pub filename: String,
    /// Unix timestamp (seconds) when the snapshot was taken
    #[specta(type = u32)]
    pub timestamp: u64,
    /// Size of the snapshot in bytes
    #[specta(type = u32)]
    pub size: u64,
}

/// Build the history directory path for a given file.
/// e.g. vault/.margin/history/notes/foo.md/
fn history_dir(vault_path: &str, file_path: &str) -> Result<String, String> {
    let vault = normalise_slashes(vault_path);
    let file = normalise_slashes(file_path);
    let rel =
        rel_under_vault(&vault, &file).ok_or_else(|| "File is not inside the vault".to_string())?;
    reject_parent_dir(&rel)?;
    let vault = vault.trim_end_matches('/');
    Ok(format!("{vault}/.margin/history/{rel}"))
}

const HOUR_MS: u64 = 60 * 60 * 1000;
const DAY_MS: u64 = 24 * HOUR_MS;
const MONTH_MS: u64 = 30 * DAY_MS;
const WEEK_MS: u64 = 7 * DAY_MS;

/// Hard ceiling on the snapshots kept for one note, applied after the age
/// buckets. Those buckets bound a note edited for years; this bounds a note
/// rewritten thousands of times inside the first hour.
pub(crate) const RETENTION_HARD_CAP: usize = 100;

/// Filenames are `<unix_millis>.<ext>`. Timestamps written by earlier builds
/// carry unix *seconds*, an order of magnitude (and then some) below the
/// millisecond range, so the magnitude tells the two apart.
fn snapshot_millis(filename: &str) -> Option<u64> {
    const MILLIS_THRESHOLD: u64 = 100_000_000_000;
    let stem = filename.split('.').next()?;
    let timestamp: u64 = stem.parse().ok()?;
    Some(if timestamp < MILLIS_THRESHOLD {
        timestamp * 1000
    } else {
        timestamp
    })
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Snapshot file names in `dir` with their millisecond timestamps.
fn snapshot_files(dir: &Path) -> Vec<(u64, String)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            snapshot_millis(&name).map(|ts| (ts, name))
        })
        .collect()
}

/// Which of `stamps` (milliseconds) survive retention, as indices into it.
///
/// All snapshots younger than an hour are kept, because that is the window in
/// which a mistake is still being noticed. Past that the newest snapshot per
/// hour, then per day, then per week survives; then the oldest are dropped
/// until the hard cap holds.
fn retained_indices(stamps: &[u64], now_ms: u64) -> Vec<usize> {
    let mut keep: Vec<usize> = Vec::new();
    // bucket → index of the newest snapshot seen in it
    let mut buckets: [HashMap<u64, usize>; 3] = Default::default();

    for (i, &ts) in stamps.iter().enumerate() {
        let age = now_ms.saturating_sub(ts);
        if age < HOUR_MS {
            keep.push(i);
            continue;
        }
        let (slot, width) = if age < DAY_MS {
            (0, HOUR_MS)
        } else if age < MONTH_MS {
            (1, DAY_MS)
        } else {
            (2, WEEK_MS)
        };
        let bucket = buckets[slot].entry(ts / width).or_insert(i);
        if ts > stamps[*bucket] {
            *bucket = i;
        }
    }
    for bucket in buckets {
        keep.extend(bucket.into_values());
    }

    keep.sort_by_key(|&i| stamps[i]);
    if keep.len() > RETENTION_HARD_CAP {
        keep.drain(..keep.len() - RETENTION_HARD_CAP);
    }
    keep.sort_unstable();
    keep
}

/// Drop the snapshots `retained_indices` leaves out of `dir`.
fn prune_snapshots(dir: &Path) {
    prune_snapshots_at(dir, now_millis())
}

fn prune_snapshots_at(dir: &Path, now_ms: u64) {
    let files = snapshot_files(dir);
    let stamps: Vec<u64> = files.iter().map(|(ts, _)| *ts).collect();
    let kept = retained_indices(&stamps, now_ms);
    for (i, (_, name)) in files.into_iter().enumerate() {
        if !kept.contains(&i) {
            let _ = fs::remove_file(dir.join(name));
        }
    }
}

/// Save a snapshot of `content` for the file at `file_path`.
///
/// Skips the write when the newest snapshot already holds these exact bytes: a
/// snapshot records a state the note has *left*, so re-recording the state it is
/// still in is churn, not history.
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

    // Use the same extension as the original file
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("md");

    // Millisecond names, unique: two panes holding the same note have
    // independent timers and can save within the same millisecond.
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

/// Save a snapshot of the given file content.
#[tauri::command]
#[specta::specta]
pub fn save_snapshot(
    vault_path: &str,
    file_path: &str,
    content: Vec<u8>,
) -> Result<String, String> {
    save_snapshot_inner(vault_path, file_path, &content)
}

/// List all snapshots for a given file, sorted newest-first.
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

/// Resolve a snapshot path and verify it stays within the history directory (prevents symlink attacks).
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
#[specta::specta]
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
#[specta::specta]
pub fn delete_snapshot(
    vault_path: &str,
    file_path: &str,
    snapshot_filename: &str,
) -> Result<(), String> {
    let snapshot_path = safe_snapshot_path(vault_path, file_path, snapshot_filename)?;
    fs::remove_file(&snapshot_path).map_err(|e| format!("Failed to delete snapshot: {e}"))
}

/// Delete all snapshots for a given file. Returns the count as a u32 (not u64)
/// so specta can export it; retention caps a note at `RETENTION_HARD_CAP`
/// snapshots, so the count never approaches u32::MAX.
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

    // Remove the directory itself if empty
    let _ = fs::remove_dir(dir_path);

    Ok(count)
}

/// Move/rename the history directory when a file or directory is renamed.
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

/// Move every snapshot under `src` into `dst`, creating `dst` when absent.
///
/// A plain directory rename fails when the destination already exists (an
/// earlier copy of the note left a history there), which left the snapshots
/// orphaned. Merging instead keeps both sets.
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

    // Succeeds only once the directory is empty; a failure just leaves it for
    // the orphan sweep on the next vault open.
    let _ = fs::remove_dir(src);
    Ok(())
}

/// A free name in `dir` for a colliding snapshot: the timestamp is bumped until
/// it is unused, so the name stays parseable (`<millis>.<ext>`) and the two sets
/// merge into one timeline instead of one shadowing the other.
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

/// Remove history directories whose note no longer exists in the vault.
///
/// Renames, sync deletes and failed renames can all leave history behind with
/// nothing left to restore it onto. Best-effort: a directory that cannot be read
/// is left alone.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_vault(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "margin-history-test-{}-{tag}-{:?}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_snapshot_is_not_repeated_while_the_note_holds_the_same_bytes() {
        let vault = temp_vault("dedupe");
        let root = vault.to_string_lossy().into_owned();
        let note = vault.join("note.md");
        fs::write(&note, "current").unwrap();
        let note = note.to_string_lossy().into_owned();

        save_snapshot_inner(&root, &note, b"previous").unwrap();
        save_snapshot_inner(&root, &note, b"previous").unwrap();
        save_snapshot_inner(&root, &note, b"previous").unwrap();

        let dir = vault.join(".margin/history/note.md");
        assert_eq!(names(&dir).len(), 1, "identical states must not pile up");

        // A different departed state still lands.
        save_snapshot_inner(&root, &note, b"other").unwrap();
        assert_eq!(names(&dir).len(), 2);

        fs::remove_dir_all(&vault).ok();
    }

    #[test]
    fn a_snapshot_taken_in_the_same_millisecond_still_gets_its_own_file() {
        let vault = temp_vault("mills");
        let root = vault.to_string_lossy().into_owned();
        let note = vault.join("note.md");
        fs::write(&note, "x").unwrap();
        let note = note.to_string_lossy().into_owned();

        save_snapshot_inner(&root, &note, b"one").unwrap();
        save_snapshot_inner(&root, &note, b"two").unwrap();

        let dir = vault.join(".margin/history/note.md");
        assert_eq!(names(&dir).len(), 2);

        fs::remove_dir_all(&vault).ok();
    }

    #[test]
    fn retention_keeps_the_recent_hour_and_one_newest_per_wider_bucket() {
        // Aligned to the week, so each pair below provably shares a bucket.
        let now = 1000 * WEEK_MS;
        let stamps = vec![
            now - 1_000,                     // inside the hour: kept by age
            now - 59 * 60 * 1000,            // inside the hour: kept by age
            now - 2 * HOUR_MS + 20 * 60_000, // hour bucket, newer
            now - 2 * HOUR_MS + 10 * 60_000, // same hour bucket, older → dropped
            now - 2 * DAY_MS + 2 * HOUR_MS,  // day bucket, newer
            now - 2 * DAY_MS + HOUR_MS,      // same day bucket, older → dropped
            now - 40 * DAY_MS + 2 * HOUR_MS, // week bucket, newer
            now - 40 * DAY_MS + HOUR_MS,     // same week bucket, older → dropped
        ];

        let kept: Vec<u64> = retained_indices(&stamps, now)
            .into_iter()
            .map(|i| stamps[i])
            .collect();

        assert_eq!(
            kept,
            vec![
                now - 1_000,
                now - 59 * 60 * 1000,
                now - 2 * HOUR_MS + 20 * 60_000,
                now - 2 * DAY_MS + 2 * HOUR_MS,
                now - 40 * DAY_MS + 2 * HOUR_MS
            ]
        );
    }

    #[test]
    fn retention_hard_caps_a_note_rewritten_all_hour() {
        let now = 1_000_000_000_000u64;
        // One a second for three hours: every one inside the first hour would
        // survive by age alone, so the cap is what bounds the directory.
        let stamps: Vec<u64> = (0..3 * 3600u64).map(|s| now - s * 1000).collect();

        let kept = retained_indices(&stamps, now);
        assert_eq!(kept.len(), RETENTION_HARD_CAP);

        // The survivors are the newest ones.
        for &i in &kept {
            assert!(
                stamps[i] >= stamps[stamps.len() - RETENTION_HARD_CAP],
                "an old snapshot survived the cap"
            );
        }
    }

    #[test]
    fn prune_snapshots_removes_only_what_retention_drops() {
        let vault = temp_vault("prune");
        let dir = vault.join("history/note.md");
        fs::create_dir_all(&dir).unwrap();
        let now = 1000 * WEEK_MS;
        let kept = [now - 2_000, now - 1_000, now - 2 * DAY_MS + 2 * HOUR_MS];
        let dropped = [now - 2 * DAY_MS + HOUR_MS];
        for ts in kept.iter().chain(dropped.iter()) {
            fs::write(dir.join(format!("{ts}.md")), "x").unwrap();
        }

        prune_snapshots_at(&dir, now);

        let left = names(&dir);
        for ts in dropped {
            assert!(
                !left.contains(&format!("{ts}.md")),
                "{ts} should have been pruned"
            );
        }
        for ts in kept {
            assert!(
                left.contains(&format!("{ts}.md")),
                "{ts} should have survived"
            );
        }

        fs::remove_dir_all(&vault).ok();
    }

    #[test]
    fn orphan_history_is_swept_and_live_history_is_kept() {
        let vault = temp_vault("orphans");
        let root = vault.to_string_lossy().into_owned();
        let history = vault.join(".margin/history");
        fs::create_dir_all(history.join("notes/todo.md")).unwrap();
        fs::create_dir_all(history.join("notes/gone.md")).unwrap();
        fs::write(history.join("notes/todo.md/1712928000123.md"), "x").unwrap();
        fs::write(history.join("notes/gone.md/1712928000123.md"), "x").unwrap();
        fs::create_dir_all(vault.join("notes")).unwrap();
        fs::write(vault.join("notes/todo.md"), "live").unwrap();

        assert_eq!(prune_orphans(&root), 1);

        assert!(
            history.join("notes/todo.md").exists(),
            "live note keeps history"
        );
        assert!(!history.join("notes/gone.md").exists());

        fs::remove_dir_all(&vault).ok();
    }

    #[test]
    fn merging_history_keeps_both_sets_and_never_overwrites() {
        let vault = temp_vault("merge");
        let src = vault.join("old/note.md");
        let dst = vault.join("new/note.md");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("1712928000000.md"), "from old").unwrap();
        fs::write(src.join("1712928000001.md"), "only old had this").unwrap();
        fs::write(dst.join("1712928000000.md"), "from new").unwrap();

        merge_history_dirs(&src, &dst).unwrap();

        assert!(
            !src.exists(),
            "the source directory should be gone once empty"
        );
        assert_eq!(names(&dst).len(), 3);
        assert_eq!(
            fs::read_to_string(dst.join("1712928000000.md")).unwrap(),
            "from new",
            "the destination's own snapshot must survive"
        );
        let merged = names(&dst)
            .into_iter()
            .filter(|n| fs::read_to_string(dst.join(n)).unwrap() == "only old had this")
            .count();
        assert_eq!(merged, 1, "the colliding snapshot landed under a new name");

        fs::remove_dir_all(&vault).ok();
    }

    #[test]
    fn merging_history_recurses_into_subdirectories() {
        let vault = temp_vault("merge-dir");
        let src = vault.join("old");
        let dst = vault.join("new");
        fs::create_dir_all(src.join("sub/a.md")).unwrap();
        fs::create_dir_all(dst.join("sub/a.md")).unwrap();
        fs::write(src.join("sub/a.md/1.md"), "a").unwrap();
        fs::write(dst.join("sub/a.md/2.md"), "b").unwrap();

        merge_history_dirs(&src, &dst).unwrap();

        assert_eq!(names(&dst.join("sub/a.md")).len(), 2);
        assert!(!src.exists());

        fs::remove_dir_all(&vault).ok();
    }
}
