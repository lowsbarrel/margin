use std::collections::{BTreeMap, BTreeSet};

use margin_dates::{CivilDateTime, UtcOffset};
use margin_kernel::{DirEntry, EntryKind, FileSystem, RelativePath, StoreError, UnixMillis};
use serde::{Deserialize, Serialize};

use crate::error::Error;

pub const MANIFEST_VERSION: u64 = 3;
pub(crate) const MANIFEST_PATH: &str = ".margin/sync-base.enc";
pub(crate) const MAX_WALK_DEPTH: usize = 10;
const TOMBSTONE_TTL_SECONDS: u64 = 90 * 24 * 60 * 60;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: String,
    pub hash: String,
    pub modified: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u64,
    pub files: Vec<ManifestEntry>,
}

impl Manifest {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            version: MANIFEST_VERSION,
            files: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncActionKind {
    Upload,
    Download,
    DeleteRemote,
    DeleteLocal,
    Conflict,
    ConflictDeleteLocal,
    ConflictDeleteRemote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncAction {
    pub kind: SyncActionKind,
    pub path: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConflictStrategy {
    #[default]
    LocalWins,
    KeepNewer,
}

#[must_use]
pub fn compute_actions(
    base_files: &[ManifestEntry],
    local_files: &[ManifestEntry],
    remote_files: &[ManifestEntry],
    now_seconds: u64,
) -> Vec<SyncAction> {
    let base = index(base_files);
    let local = index(local_files);
    let remote = index(remote_files);

    let mut paths: BTreeSet<&str> = BTreeSet::new();
    paths.extend(base.keys().copied());
    paths.extend(local.keys().copied());
    paths.extend(remote.keys().copied());

    let horizon = now_seconds.saturating_sub(TOMBSTONE_TTL_SECONDS);
    let mut actions = Vec::new();
    for path in paths {
        let base_hash = effective_hash(&base, path);
        let local_hash = effective_hash(&local, path);
        let remote_absent = !remote.contains_key(path);
        let remote_hash = if remote_absent {
            None
        } else {
            effective_hash(&remote, path)
        };
        if local_hash == remote_hash {
            continue;
        }

        let kind = if base_hash.is_none() {
            match (local_hash, remote_hash) {
                (Some(_), None) => SyncActionKind::Upload,
                (None, Some(_)) => SyncActionKind::Download,
                _ => SyncActionKind::Conflict,
            }
        } else {
            let local_changed = local_hash != base_hash;
            let remote_changed = remote_hash != base_hash;
            match (local_hash, remote_hash) {
                (None, None) => continue,
                (None, _) => {
                    if remote_changed {
                        SyncActionKind::ConflictDeleteLocal
                    } else {
                        SyncActionKind::DeleteRemote
                    }
                }
                (_, None) => {
                    let recent = remote_absent
                        && base.get(path).is_some_and(|entry| entry.modified > horizon);
                    if recent {
                        SyncActionKind::Upload
                    } else if local_changed {
                        SyncActionKind::ConflictDeleteRemote
                    } else {
                        SyncActionKind::DeleteLocal
                    }
                }
                _ if local_changed && !remote_changed => SyncActionKind::Upload,
                _ if !local_changed && remote_changed => SyncActionKind::Download,
                _ => SyncActionKind::Conflict,
            }
        };
        actions.push(SyncAction {
            kind,
            path: path.to_owned(),
        });
    }
    actions
}

fn effective_hash<'a>(map: &'a BTreeMap<&'a str, &'a ManifestEntry>, key: &str) -> Option<&'a str> {
    map.get(key)
        .filter(|entry| entry.deleted_at.is_none())
        .map(|entry| entry.hash.as_str())
}

#[must_use]
pub fn collect_tombstones(files: &[ManifestEntry]) -> Vec<ManifestEntry> {
    files
        .iter()
        .filter(|entry| entry.deleted_at.is_some())
        .cloned()
        .collect()
}

#[must_use]
pub fn merge_tombstones(a: &[ManifestEntry], b: &[ManifestEntry]) -> Vec<ManifestEntry> {
    let mut merged: BTreeMap<&str, &ManifestEntry> = BTreeMap::new();
    for entry in a.iter().chain(b) {
        let newer = merged.get(entry.path.as_str()).is_none_or(|existing| {
            entry.deleted_at.unwrap_or(0) > existing.deleted_at.unwrap_or(0)
        });
        if newer {
            merged.insert(entry.path.as_str(), entry);
        }
    }
    merged.into_values().cloned().collect()
}

#[must_use]
pub fn prune_tombstones(tombstones: &[ManifestEntry], now_seconds: u64) -> Vec<ManifestEntry> {
    let cutoff = now_seconds.saturating_sub(TOMBSTONE_TTL_SECONDS);
    tombstones
        .iter()
        .filter(|entry| entry.deleted_at.unwrap_or(0) > cutoff)
        .cloned()
        .collect()
}

#[must_use]
pub fn conflict_copy_name(path: &str, now: UnixMillis, sequence: u64) -> String {
    let at = CivilDateTime::from_unix_millis(now, UtcOffset::UTC);
    let suffix = format!(
        "sync-conflict-{:04}{:02}{:02}-{:02}{:02}{:02}-{sequence}",
        at.year, at.month, at.day, at.hour, at.minute, at.second
    );
    let name_start = path.rfind('/').map_or(0, |slash| slash + 1);
    match path[name_start..].rfind('.') {
        Some(dot) if dot > 0 => {
            let (stem, extension) = path.split_at(name_start + dot);
            format!("{stem}.{suffix}{extension}")
        }
        _ => format!("{path}.{suffix}"),
    }
}

pub(crate) fn merged_manifest(
    merged: &BTreeMap<String, ManifestEntry>,
    tombstones: &BTreeMap<String, ManifestEntry>,
    now_seconds: u64,
) -> Manifest {
    let tombstones: Vec<ManifestEntry> = tombstones.values().cloned().collect();
    let mut files: Vec<ManifestEntry> = merged.values().cloned().collect();
    files.extend(prune_tombstones(&tombstones, now_seconds));
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Manifest {
        version: MANIFEST_VERSION,
        files,
    }
}

pub(crate) fn with_missing(
    manifest: &Manifest,
    missing: &BTreeMap<String, ManifestEntry>,
) -> Manifest {
    let mut files = manifest.files.clone();
    files.extend(missing.values().cloned());
    Manifest {
        version: MANIFEST_VERSION,
        files,
    }
}

pub(crate) fn encode_manifest(manifest: &Manifest) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(manifest).map_err(|error| Error::Manifest(error.to_string()))
}

pub(crate) fn decode_manifest(bytes: &[u8]) -> Result<Manifest, Error> {
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|error| Error::Manifest(error.to_string()))?;
    Ok(if manifest.version < MANIFEST_VERSION {
        Manifest::empty()
    } else {
        manifest
    })
}

pub(crate) fn index(files: &[ManifestEntry]) -> BTreeMap<&str, &ManifestEntry> {
    files
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect()
}

pub(crate) fn count<T>(items: &[T]) -> u64 {
    u64::try_from(items.len()).unwrap_or(u64::MAX)
}

pub(crate) fn relative(path: &str) -> Result<RelativePath, Error> {
    Ok(RelativePath::new(path)?)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub(crate) fn walk(files: &dyn FileSystem) -> Result<Vec<(RelativePath, u64)>, Error> {
    let mut found = Vec::new();
    walk_dir(files, &RelativePath::root(), 0, &mut found)?;
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

fn walk_dir(
    files: &dyn FileSystem,
    dir: &RelativePath,
    depth: usize,
    out: &mut Vec<(RelativePath, u64)>,
) -> Result<(), Error> {
    if depth >= MAX_WALK_DEPTH {
        return Ok(());
    }
    let entries: Vec<DirEntry> = match files.list(dir) {
        Ok(entries) => entries,
        Err(StoreError::NotFound(_)) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        if entry.name.starts_with('.') {
            continue;
        }
        let path = dir.join(&entry.name)?;
        match entry.metadata.kind {
            EntryKind::Directory => walk_dir(files, &path, depth + 1, out)?,
            EntryKind::File => out.push((path, entry.metadata.modified.as_seconds())),
            EntryKind::Symlink => {}
        }
    }
    Ok(())
}

pub(crate) fn build_local_manifest(
    base: &Manifest,
    files: &dyn FileSystem,
) -> Result<Manifest, Error> {
    let known = index(&base.files);
    let mut entries = Vec::new();
    let mut to_hash = Vec::new();
    for (path, modified) in walk(files)? {
        let key = path.as_str().to_owned();
        match known.get(key.as_str()) {
            Some(entry) if entry.deleted_at.is_none() && entry.modified == modified => {
                entries.push((*entry).clone());
            }
            _ => to_hash.push((key, path, modified)),
        }
    }
    for (key, path, modified) in to_hash {
        let bytes = files.read(&path)?;
        entries.push(ManifestEntry {
            path: key,
            hash: sha256_hex(&bytes),
            modified,
            deleted_at: None,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Manifest {
        version: MANIFEST_VERSION,
        files: entries,
    })
}
