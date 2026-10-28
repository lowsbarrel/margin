use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use margin_crypto::{DerivedVault, VaultKey};
use margin_kernel::{Clock, FileSystem, StoreError, Timer, UnixMillis};
use margin_retry::RetryPolicy;

use crate::domain::{
    ConflictStrategy, MANIFEST_PATH, Manifest, ManifestEntry, SyncActionKind, build_local_manifest,
    collect_tombstones, compute_actions, conflict_copy_name, count, decode_manifest,
    encode_manifest, index, merge_tombstones, merged_manifest, relative, walk, with_missing,
};
use crate::error::Error;
use crate::ports::{BlobStore, SnapshotStore};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncOptions {
    pub conflict_strategy: ConflictStrategy,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncProgress {
    pub total: u64,
    pub done: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub uploaded: u64,
    pub downloaded: u64,
    pub deleted: u64,
    pub conflicts: u64,
}

pub struct SyncService {
    files: Arc<dyn FileSystem>,
    blobs: Arc<dyn BlobStore>,
    clock: Arc<dyn Clock>,
    timer: Arc<dyn Timer>,
    snapshots: Arc<dyn SnapshotStore>,
    prefix: String,
    key: VaultKey,
    conflicts: AtomicU64,
}

impl fmt::Debug for SyncService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncService")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl SyncService {
    #[must_use]
    pub fn new(
        files: Arc<dyn FileSystem>,
        blobs: Arc<dyn BlobStore>,
        clock: Arc<dyn Clock>,
        timer: Arc<dyn Timer>,
        snapshots: Arc<dyn SnapshotStore>,
        vault: DerivedVault,
    ) -> Self {
        let DerivedVault { id, key } = vault;
        Self {
            files,
            blobs,
            clock,
            timer,
            snapshots,
            prefix: format!("{}/", id.as_str()),
            key,
            conflicts: AtomicU64::new(0),
        }
    }

    pub async fn test_connection(&self) -> Result<usize, Error> {
        Ok(self.retried(|| self.blobs.list_prefix("")).await?.len())
    }

    pub fn has_unsynced_changes(&self) -> Result<bool, Error> {
        let base = self.load_base_manifest()?;
        let known: BTreeMap<&str, u64> = base
            .files
            .iter()
            .filter(|entry| entry.deleted_at.is_none())
            .map(|entry| (entry.path.as_str(), entry.modified))
            .collect();
        let local = walk(self.files.as_ref())?;
        if local.len() != known.len() {
            return Ok(true);
        }
        Ok(local
            .iter()
            .any(|(path, modified)| known.get(path.as_str()) != Some(modified)))
    }

    pub async fn sync_now(
        &self,
        options: SyncOptions,
        progress: &mut (dyn FnMut(SyncProgress) + Send),
    ) -> Result<SyncReport, Error> {
        let now = self.clock.now().as_seconds();
        let base = self.load_base_manifest()?;
        let local = build_local_manifest(&base, self.files.as_ref())?;
        let remote = self.load_remote_manifest().await?;
        let actions = compute_actions(&base.files, &local.files, &remote.files, now);

        let total = count(&actions);
        progress(SyncProgress { total, done: 0 });

        let base_map = index(&base.files);
        let local_map = index(&local.files);
        let remote_map = index(&remote.files);
        let mut state = State {
            merged: local
                .files
                .iter()
                .map(|entry| (entry.path.clone(), entry.clone()))
                .collect(),
            tombstones: merge_tombstones(
                &collect_tombstones(&base.files),
                &collect_tombstones(&remote.files),
            )
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect(),
            missing: BTreeMap::new(),
        };
        let mut report = SyncReport::default();

        for (done, action) in (1..).zip(&actions) {
            match action.kind {
                SyncActionKind::Upload | SyncActionKind::ConflictDeleteRemote => {
                    self.upload_entry(&action.path, &mut state).await?;
                    report.uploaded += 1;
                }
                SyncActionKind::Download | SyncActionKind::ConflictDeleteLocal => {
                    if let Some(entry) = remote_map.get(action.path.as_str())
                        && self.download_entry(entry, &mut state).await?
                    {
                        report.downloaded += 1;
                    }
                }
                SyncActionKind::DeleteRemote => {
                    self.delete_remote_entry(&action.path).await?;
                    state.mark_tombstone(&action.path, &base_map, now);
                    report.deleted += 1;
                }
                SyncActionKind::DeleteLocal => {
                    self.delete_local_entry(&action.path)?;
                    state.mark_tombstone(&action.path, &remote_map, now);
                    report.deleted += 1;
                }
                SyncActionKind::Conflict => {
                    if let (Some(local), Some(remote)) = (
                        local_map.get(action.path.as_str()),
                        remote_map.get(action.path.as_str()),
                    ) {
                        let strategy = options.conflict_strategy;
                        self.resolve_conflict(
                            &action.path,
                            local,
                            remote,
                            strategy,
                            now,
                            &mut state,
                        )
                        .await?;
                    }
                }
            }
            if matches!(
                action.kind,
                SyncActionKind::Conflict
                    | SyncActionKind::ConflictDeleteLocal
                    | SyncActionKind::ConflictDeleteRemote
            ) {
                report.conflicts += 1;
            }
            progress(SyncProgress { total, done });
        }

        let local_manifest = merged_manifest(&state.merged, &state.tombstones, now);
        self.upload_manifest(&with_missing(&local_manifest, &state.missing))
            .await?;
        self.save_base_manifest(&local_manifest)?;
        Ok(report)
    }

    async fn retried<T, Call, Attempt>(&self, mut call: Call) -> Result<T, Error>
    where
        Call: FnMut() -> Attempt,
        Attempt: Future<Output = Result<T, StoreError>>,
    {
        margin_retry::retry(
            &RetryPolicy::default(),
            |delay| self.timer.sleep(delay),
            || {
                let attempt = call();
                async move { attempt.await.map_err(Error::from) }
            },
        )
        .await
    }

    fn load_base_manifest(&self) -> Result<Manifest, Error> {
        match self.files.read(&relative(MANIFEST_PATH)?) {
            Ok(bytes) => decode_manifest(&margin_crypto::decrypt(&self.key, &bytes)?),
            Err(StoreError::NotFound(_)) => Ok(Manifest::empty()),
            Err(error) => Err(error.into()),
        }
    }

    fn save_base_manifest(&self, manifest: &Manifest) -> Result<(), Error> {
        let blob = margin_crypto::encrypt(&self.key, &encode_manifest(manifest)?);
        self.files.write(&relative(MANIFEST_PATH)?, &blob)?;
        Ok(())
    }

    async fn load_remote_manifest(&self) -> Result<Manifest, Error> {
        let key = self.manifest_key();
        match self.retried(|| self.blobs.get(&key)).await {
            Ok(bytes) => decode_manifest(&margin_crypto::decrypt(&self.key, &bytes)?),
            Err(Error::Store(StoreError::NotFound(_))) => Ok(Manifest::empty()),
            Err(error) => Err(error),
        }
    }

    async fn upload_manifest(&self, manifest: &Manifest) -> Result<(), Error> {
        let blob = margin_crypto::encrypt(&self.key, &encode_manifest(manifest)?);
        let key = self.manifest_key();
        self.retried(|| self.blobs.put(&key, blob.clone())).await
    }

    async fn upload_entry(&self, path: &str, state: &mut State) -> Result<(), Error> {
        let blob = margin_crypto::encrypt(&self.key, &self.files.read(&relative(path)?)?);
        let key = self.file_object_key(path);
        self.retried(|| self.blobs.put(&key, blob.clone())).await?;
        state.tombstones.remove(path);
        Ok(())
    }

    async fn download_entry(
        &self,
        target: &ManifestEntry,
        state: &mut State,
    ) -> Result<bool, Error> {
        let rel = relative(&target.path)?;
        let key = self.file_object_key(&target.path);
        let bytes = match self.retried(|| self.blobs.get(&key)).await {
            Ok(bytes) => bytes,
            Err(Error::Store(StoreError::NotFound(_))) => {
                state.missing.insert(target.path.clone(), target.clone());
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        let decoded = margin_crypto::decrypt(&self.key, &bytes)?;
        match self.files.read(&rel) {
            Ok(existing) if existing != decoded => self.snapshots.save(&rel, &existing)?,
            Ok(_) | Err(StoreError::NotFound(_)) => {}
            Err(error) => return Err(error.into()),
        }
        self.files.write(&rel, &decoded)?;
        self.files
            .set_modified(&rel, UnixMillis::from_seconds(target.modified))?;
        state.tombstones.remove(&target.path);
        state.merged.insert(target.path.clone(), target.clone());
        Ok(true)
    }

    async fn delete_remote_entry(&self, path: &str) -> Result<(), Error> {
        let key = self.file_object_key(path);
        self.retried(|| self.blobs.delete(&key)).await
    }

    fn delete_local_entry(&self, path: &str) -> Result<(), Error> {
        match self.files.remove_file(&relative(path)?) {
            Ok(()) | Err(StoreError::NotFound(_)) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    async fn resolve_conflict(
        &self,
        path: &str,
        local: &ManifestEntry,
        remote: &ManifestEntry,
        strategy: ConflictStrategy,
        now: u64,
        state: &mut State,
    ) -> Result<(), Error> {
        let sequence = self.conflicts.fetch_add(1, Ordering::Relaxed) + 1;
        let copy_path = conflict_copy_name(path, self.clock.now(), sequence);
        let remote_wins =
            strategy == ConflictStrategy::KeepNewer && remote.modified > local.modified;

        if remote_wins {
            let local_bytes = self.files.read(&relative(path)?)?;
            self.write_copy(&copy_path, &local_bytes, now)?;
            self.record_conflict(copy_path, &local.hash, now, state)
                .await?;
            self.download_entry(remote, state).await?;
        } else {
            self.write_remote_copy(path, &copy_path, now).await?;
            self.record_conflict(copy_path, &remote.hash, now, state)
                .await?;
            self.upload_entry(path, state).await?;
        }
        state.tombstones.remove(path);
        Ok(())
    }

    async fn record_conflict(
        &self,
        path: String,
        hash: &str,
        now: u64,
        state: &mut State,
    ) -> Result<(), Error> {
        self.upload_entry(&path, state).await?;
        let entry = ManifestEntry {
            path: path.clone(),
            hash: hash.to_owned(),
            modified: now,
            deleted_at: None,
        };
        state.merged.insert(path, entry);
        Ok(())
    }

    async fn write_remote_copy(
        &self,
        source: &str,
        dest: &str,
        modified: u64,
    ) -> Result<(), Error> {
        let key = self.file_object_key(source);
        let bytes = self.retried(|| self.blobs.get(&key)).await?;
        let decoded = margin_crypto::decrypt(&self.key, &bytes)?;
        self.write_copy(dest, &decoded, modified)
    }

    fn write_copy(&self, path: &str, contents: &[u8], modified: u64) -> Result<(), Error> {
        let rel = relative(path)?;
        self.files.write(&rel, contents)?;
        self.files
            .set_modified(&rel, UnixMillis::from_seconds(modified))?;
        Ok(())
    }

    fn file_object_key(&self, path: &str) -> String {
        format!(
            "{}files/{}.enc",
            self.prefix,
            margin_crypto::object_key(&self.key, path)
        )
    }

    fn manifest_key(&self) -> String {
        format!("{}manifest.enc", self.prefix)
    }
}

struct State {
    merged: BTreeMap<String, ManifestEntry>,
    tombstones: BTreeMap<String, ManifestEntry>,
    missing: BTreeMap<String, ManifestEntry>,
}

impl State {
    fn mark_tombstone(&mut self, path: &str, fallback: &BTreeMap<&str, &ManifestEntry>, now: u64) {
        let existing = self
            .merged
            .get(path)
            .cloned()
            .or_else(|| fallback.get(path).map(|entry| (*entry).clone()));
        if let Some(entry) = existing {
            self.tombstones.insert(
                path.to_owned(),
                ManifestEntry {
                    deleted_at: Some(now),
                    ..entry
                },
            );
        }
        self.merged.remove(path);
    }
}
