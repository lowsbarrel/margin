use super::{ManifestEntry, SyncAction};
use std::collections::{HashMap, HashSet};

const TOMBSTONE_TTL: u64 = 90 * 24 * 60 * 60;

pub(super) fn compute(
    base_files: Vec<ManifestEntry>,
    local_files: Vec<ManifestEntry>,
    remote_files: Vec<ManifestEntry>,
    now_seconds: u32,
) -> Vec<SyncAction> {
    let base: HashMap<&str, &ManifestEntry> =
        base_files.iter().map(|e| (e.path.as_str(), e)).collect();
    let local: HashMap<&str, &ManifestEntry> =
        local_files.iter().map(|e| (e.path.as_str(), e)).collect();
    let remote: HashMap<&str, &ManifestEntry> =
        remote_files.iter().map(|e| (e.path.as_str(), e)).collect();

    let all_paths: HashSet<&str> = base
        .keys()
        .chain(local.keys())
        .chain(remote.keys())
        .copied()
        .collect();

    let effective_hash = |map: &HashMap<&str, &ManifestEntry>, path: &str| -> Option<String> {
        match map.get(path) {
            Some(e) if e.deleted_at.is_none() => Some(e.hash.clone()),
            _ => None,
        }
    };

    let tombstone_horizon = (now_seconds as u64).saturating_sub(TOMBSTONE_TTL);

    let mut actions = Vec::new();
    for path in all_paths {
        let base_h = effective_hash(&base, path);
        let local_h = effective_hash(&local, path);
        // A path with no remote entry at all is not a deletion: deletions always leave a tombstone.
        let remote_absent = !remote.contains_key(path);
        let remote_h = if remote_absent {
            None
        } else {
            effective_hash(&remote, path)
        };

        if local_h == remote_h {
            continue;
        }

        let kind = if base_h.is_none() {
            match (local_h, remote_h) {
                (Some(_), None) => "upload",
                (None, Some(_)) => "download",
                _ => "conflict",
            }
        } else {
            let local_changed = local_h != base_h;
            let remote_changed = remote_h != base_h;

            match (local_h, remote_h) {
                (None, None) => continue,
                (None, _) => {
                    if remote_changed {
                        "conflict-delete-local"
                    } else {
                        "delete-remote"
                    }
                }
                (_, None) => {
                    let base_recent = base
                        .get(path)
                        .is_some_and(|e| e.modified > tombstone_horizon);
                    if remote_absent && base_recent {
                        "upload"
                    } else if local_changed {
                        "conflict-delete-remote"
                    } else {
                        "delete-local"
                    }
                }
                _ => {
                    if local_changed && !remote_changed {
                        "upload"
                    } else if !local_changed && remote_changed {
                        "download"
                    } else {
                        "conflict"
                    }
                }
            }
        };

        actions.push(SyncAction {
            kind: kind.to_string(),
            path: path.to_string(),
        });
    }

    actions
}

pub(super) fn collect_tombstones(files: Vec<ManifestEntry>) -> Vec<ManifestEntry> {
    files
        .into_iter()
        .filter(|e| e.deleted_at.is_some())
        .collect()
}

pub(super) fn merge_tombstones(a: Vec<ManifestEntry>, b: Vec<ManifestEntry>) -> Vec<ManifestEntry> {
    let mut map: HashMap<String, ManifestEntry> = HashMap::new();
    for entry in a.into_iter().chain(b) {
        let existing = map.get(&entry.path);
        if existing.is_none()
            || entry.deleted_at.unwrap_or(0) > existing.unwrap().deleted_at.unwrap_or(0)
        {
            map.insert(entry.path.clone(), entry);
        }
    }
    map.into_values().collect()
}

pub(super) fn prune_tombstones(
    tombstones: Vec<ManifestEntry>,
    now_seconds: u32,
) -> Vec<ManifestEntry> {
    let cutoff = (now_seconds as u64).saturating_sub(TOMBSTONE_TTL);
    tombstones
        .into_iter()
        .filter(|t| t.deleted_at.unwrap_or(0) > cutoff)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, hash: &str, modified: u64) -> ManifestEntry {
        ManifestEntry {
            path: path.into(),
            hash: hash.into(),
            modified,
            deleted_at: None,
        }
    }

    fn tombstone(path: &str, hash: &str, modified: u64, deleted_at: u64) -> ManifestEntry {
        ManifestEntry {
            path: path.into(),
            hash: hash.into(),
            modified,
            deleted_at: Some(deleted_at),
        }
    }

    fn kind_of(actions: &[SyncAction], path: &str) -> Option<String> {
        actions
            .iter()
            .find(|a| a.path == path)
            .map(|a| a.kind.clone())
    }

    const NOW: u32 = 2_000_000_000;

    // A path with no remote entry at all is a lost write, never a deletion: uploading keeps the note.
    #[test]
    fn absent_remote_entry_is_uploaded_not_deleted() {
        let actions = compute(
            vec![entry("x.md", "h1", NOW as u64)],
            vec![entry("x.md", "h1", NOW as u64)],
            vec![],
            NOW,
        );
        assert_eq!(kind_of(&actions, "x.md").as_deref(), Some("upload"));
    }

    #[test]
    fn absent_remote_entry_with_local_change_is_uploaded() {
        let actions = compute(
            vec![entry("x.md", "h1", NOW as u64)],
            vec![entry("x.md", "h2", NOW as u64 + 1)],
            vec![],
            NOW,
        );
        assert_eq!(kind_of(&actions, "x.md").as_deref(), Some("upload"));
    }

    #[test]
    fn pruned_tombstone_deletes_local() {
        let old = (NOW as u64) - TOMBSTONE_TTL - 10;
        let actions = compute(
            vec![entry("x.md", "h1", old)],
            vec![entry("x.md", "h1", old)],
            vec![],
            NOW,
        );
        assert_eq!(kind_of(&actions, "x.md").as_deref(), Some("delete-local"));
    }

    #[test]
    fn pruned_tombstone_with_local_change_is_conflict() {
        let old = (NOW as u64) - TOMBSTONE_TTL - 10;
        let actions = compute(
            vec![entry("x.md", "h1", old)],
            vec![entry("x.md", "h2", old + 5)],
            vec![],
            NOW,
        );
        assert_eq!(
            kind_of(&actions, "x.md").as_deref(),
            Some("conflict-delete-remote")
        );
    }

    #[test]
    fn tombstoned_remote_entry_deletes_local() {
        let actions = compute(
            vec![entry("x.md", "h1", 1)],
            vec![entry("x.md", "h1", 1)],
            vec![tombstone("x.md", "h1", 1, 5)],
            NOW,
        );
        assert_eq!(kind_of(&actions, "x.md").as_deref(), Some("delete-local"));
    }

    #[test]
    fn tombstoned_remote_entry_with_local_change_is_conflict() {
        let actions = compute(
            vec![entry("x.md", "h1", 1)],
            vec![entry("x.md", "h2", 2)],
            vec![tombstone("x.md", "h1", 1, 5)],
            NOW,
        );
        assert_eq!(
            kind_of(&actions, "x.md").as_deref(),
            Some("conflict-delete-remote")
        );
    }

    #[test]
    fn tombstoned_local_entry_with_remote_change_is_conflict() {
        let actions = compute(
            vec![entry("x.md", "h1", 1)],
            vec![tombstone("x.md", "h1", 1, 6)],
            vec![entry("x.md", "h2", 2)],
            NOW,
        );
        assert_eq!(
            kind_of(&actions, "x.md").as_deref(),
            Some("conflict-delete-local")
        );
    }

    #[test]
    fn unchanged_paths_produce_no_action() {
        let actions = compute(
            vec![entry("x.md", "h1", 1)],
            vec![entry("x.md", "h1", 1)],
            vec![entry("x.md", "h1", 1)],
            NOW,
        );
        assert!(actions.is_empty());
    }

    #[test]
    fn new_local_file_uploads_and_new_remote_file_downloads() {
        let actions = compute(
            vec![],
            vec![entry("new.md", "h1", 1)],
            vec![entry("other.md", "h2", 2)],
            NOW,
        );
        assert_eq!(kind_of(&actions, "new.md").as_deref(), Some("upload"));
        assert_eq!(kind_of(&actions, "other.md").as_deref(), Some("download"));
    }
}
