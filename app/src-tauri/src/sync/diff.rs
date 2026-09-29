use super::{ManifestEntry, SyncAction};
use std::collections::{HashMap, HashSet};

pub(super) fn compute(
    base_files: Vec<ManifestEntry>,
    local_files: Vec<ManifestEntry>,
    remote_files: Vec<ManifestEntry>,
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

    let mut actions = Vec::new();
    for path in all_paths {
        let base_h = effective_hash(&base, path);
        let local_h = effective_hash(&local, path);
        let remote_h = effective_hash(&remote, path);

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
                    if local_changed {
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
    const TOMBSTONE_TTL: u64 = 90 * 24 * 60 * 60;
    let cutoff = (now_seconds as u64).saturating_sub(TOMBSTONE_TTL);
    tombstones
        .into_iter()
        .filter(|t| t.deleted_at.unwrap_or(0) > cutoff)
        .collect()
}
