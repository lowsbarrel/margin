use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use margin_crypto::{DerivedVault, derive_vault};
use margin_kernel::{Clock, FileSystem, RelativePath};
use margin_sync::{
    BlobStore, ConflictStrategy, Manifest, SyncOptions, SyncProgress, SyncReport, SyncService,
};

use crate::{FixedClock, InstantTimer, MemoryFileSystem, MemorySnapshotStore};

const FIXTURE_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const NOTE: &str = "notes/a.md";
const TICK: Duration = Duration::from_secs(1);

struct Vault {
    files: Arc<MemoryFileSystem>,
    snapshots: Arc<MemorySnapshotStore>,
    service: SyncService,
}

impl Vault {
    fn new(store: &Arc<dyn BlobStore>, clock: &Arc<FixedClock>, derived: DerivedVault) -> Self {
        let files = Arc::new(MemoryFileSystem::with_clock(Arc::clone(clock)));
        let snapshots = Arc::new(MemorySnapshotStore::new());
        let service = SyncService::new(
            Arc::<MemoryFileSystem>::clone(&files),
            Arc::clone(store),
            Arc::<FixedClock>::clone(clock),
            Arc::new(InstantTimer::new()),
            Arc::<MemorySnapshotStore>::clone(&snapshots),
            derived,
        );
        Self {
            files,
            snapshots,
            service,
        }
    }

    async fn sync(&self, strategy: ConflictStrategy) -> SyncReport {
        let mut events = Vec::new();
        let options = SyncOptions {
            conflict_strategy: strategy,
        };
        let report = self
            .service
            .sync_now(options, &mut |progress: SyncProgress| events.push(progress))
            .await
            .expect("sync succeeds");
        let first = events.first().expect("sync reports its start");
        let last = events.last().expect("sync reports its end");
        assert_eq!(first.done, 0, "the first event starts at zero");
        assert_eq!(last.done, last.total, "every action is reported");
        assert!(
            events.iter().all(|event| event.total == first.total),
            "the total never changes"
        );
        report
    }

    fn text(&self, path: &str) -> Option<String> {
        self.files
            .contents(path)
            .map(|bytes| String::from_utf8(bytes).expect("fixture notes are text"))
    }

    fn visible_files(&self) -> Vec<(String, Vec<u8>)> {
        self.files
            .paths()
            .into_iter()
            .filter(|path| !path.starts_with('.'))
            .filter_map(|path| self.files.contents(&path).map(|contents| (path, contents)))
            .collect()
    }

    fn conflict_copies(&self) -> Vec<String> {
        self.files
            .paths()
            .into_iter()
            .filter(|path| path.contains(".sync-conflict-"))
            .collect()
    }
}

pub async fn sync_round_trip(store: Arc<dyn BlobStore>) {
    let derived = derive_vault(FIXTURE_MNEMONIC).expect("the fixture mnemonic derives a vault");
    let clock = Arc::new(FixedClock::default());
    let a = Vault::new(&store, &clock, derived.clone());
    let b = Vault::new(&store, &clock, derived.clone());

    a_new_note_reaches_the_other_vault(&a, &b).await;
    editing_both_sides_keeps_the_local_body_and_a_remote_copy(&a, &b, &clock).await;
    keep_newer_puts_the_newer_body_in_place(&a, &b, &clock).await;
    deleting_a_note_leaves_a_tombstone(&a, &b, &clock, store.as_ref(), &derived).await;
    assert_eq!(
        a.visible_files(),
        b.visible_files(),
        "both vaults end up identical"
    );
}

async fn a_new_note_reaches_the_other_vault(a: &Vault, b: &Vault) {
    assert_eq!(
        a.service
            .test_connection()
            .await
            .expect("the store answers"),
        0,
        "an untouched bucket is empty"
    );
    a.files.insert(NOTE, b"one");
    assert!(
        a.service.has_unsynced_changes().expect("the base reads"),
        "a new note is unsynced"
    );
    let uploaded = SyncReport {
        uploaded: 1,
        ..SyncReport::default()
    };
    assert_eq!(a.sync(ConflictStrategy::LocalWins).await, uploaded);
    assert!(!a.service.has_unsynced_changes().expect("the base reads"));

    let downloaded = SyncReport {
        downloaded: 1,
        ..SyncReport::default()
    };
    assert_eq!(b.sync(ConflictStrategy::LocalWins).await, downloaded);
    assert_eq!(b.text(NOTE).as_deref(), Some("one"));
    assert_eq!(
        b.sync(ConflictStrategy::LocalWins).await,
        SyncReport::default(),
        "a synced vault has nothing left to do"
    );
}

async fn editing_both_sides_keeps_the_local_body_and_a_remote_copy(
    a: &Vault,
    b: &Vault,
    clock: &FixedClock,
) {
    clock.advance(TICK);
    a.files.insert(NOTE, b"two");
    assert_eq!(a.sync(ConflictStrategy::LocalWins).await.uploaded, 1);
    clock.advance(TICK);
    b.files.insert(NOTE, b"three");

    let report = b.sync(ConflictStrategy::LocalWins).await;
    assert_eq!(report.conflicts, 1, "both sides changed the note");
    assert_eq!(
        b.text(NOTE).as_deref(),
        Some("three"),
        "local wins keeps the local body"
    );
    let [copy]: [String; 1] = b
        .conflict_copies()
        .try_into()
        .expect("one conflict copy is written");
    assert!(
        copy.starts_with("notes/a.sync-conflict-")
            && Path::new(&copy)
                .extension()
                .is_some_and(|extension| extension == "md"),
        "the copy sits beside the note and keeps its extension: {copy}"
    );
    assert_eq!(
        b.text(&copy).as_deref(),
        Some("two"),
        "the copy holds the remote body"
    );

    let report = a.sync(ConflictStrategy::LocalWins).await;
    assert_eq!(
        report.downloaded, 2,
        "the winner and its copy reach the other vault"
    );
    assert_eq!(a.text(NOTE).as_deref(), Some("three"));
    assert_eq!(a.text(&copy).as_deref(), Some("two"));
    assert_eq!(
        a.snapshots.snapshots(NOTE),
        [b"two".to_vec()],
        "a differing local body is snapshotted before it is overwritten"
    );
    assert_eq!(b.snapshots.count(NOTE), 0);
}

async fn keep_newer_puts_the_newer_body_in_place(a: &Vault, b: &Vault, clock: &FixedClock) {
    clock.advance(TICK);
    a.files.insert(NOTE, b"a-four");
    clock.advance(TICK);
    b.files.insert(NOTE, b"b-four");
    assert_eq!(b.sync(ConflictStrategy::KeepNewer).await.uploaded, 1);

    let report = a.sync(ConflictStrategy::KeepNewer).await;
    assert_eq!(report.conflicts, 1);
    assert_eq!(
        a.text(NOTE).as_deref(),
        Some("b-four"),
        "the newer remote body wins"
    );
    assert!(
        a.conflict_copies()
            .iter()
            .any(|copy| a.text(copy).as_deref() == Some("a-four")),
        "the older local body is kept beside it"
    );
    assert_eq!(
        b.sync(ConflictStrategy::KeepNewer).await.downloaded,
        1,
        "the new copy reaches the other vault"
    );
}

async fn deleting_a_note_leaves_a_tombstone(
    a: &Vault,
    b: &Vault,
    clock: &FixedClock,
    store: &dyn BlobStore,
    derived: &DerivedVault,
) {
    clock.advance(TICK);
    a.files
        .remove_file(&RelativePath::new(NOTE).expect("the note path is valid"))
        .expect("the note is removed");
    let objects = store.list_prefix("").await.expect("the store lists").len();
    let deleted = SyncReport {
        deleted: 1,
        ..SyncReport::default()
    };
    assert_eq!(a.sync(ConflictStrategy::LocalWins).await, deleted);
    assert_eq!(
        store.list_prefix("").await.expect("the store lists").len(),
        objects - 1,
        "the note's object is deleted, not kept"
    );

    let manifest = remote_manifest(store, derived).await;
    let tombstone = manifest
        .files
        .iter()
        .find(|entry| entry.path == NOTE)
        .expect("the manifest keeps the deleted note");
    assert_eq!(tombstone.deleted_at, Some(clock.now().as_seconds()));

    assert_eq!(
        b.sync(ConflictStrategy::LocalWins).await,
        deleted,
        "the tombstone reaches the other vault"
    );
    assert_eq!(b.text(NOTE), None);
}

async fn remote_manifest(store: &dyn BlobStore, derived: &DerivedVault) -> Manifest {
    let key = format!("{}/manifest.enc", derived.id.as_str());
    let bytes = store.get(&key).await.expect("the manifest object exists");
    let plaintext = margin_crypto::decrypt(&derived.key, &bytes).expect("the manifest decrypts");
    serde_json::from_slice(&plaintext).expect("the manifest is JSON")
}
