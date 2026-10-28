use std::sync::Arc;

use margin_crypto::derive_vault;
use margin_kernel::StoreError;
use margin_sync::{BlobStore, Error, SyncOptions, SyncReport, SyncService};
use margin_test_support::{
    FixedClock, InstantTimer, MemoryBlobStore, MemoryFileSystem, MemorySnapshotStore, block_on,
    sync_round_trip,
};

const FIXTURE_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

struct Vault {
    files: Arc<MemoryFileSystem>,
    timer: Arc<InstantTimer>,
    service: SyncService,
}

impl Vault {
    fn new(store: &Arc<MemoryBlobStore>) -> Self {
        let clock = Arc::new(FixedClock::default());
        let files = Arc::new(MemoryFileSystem::with_clock(Arc::clone(&clock)));
        let timer = Arc::new(InstantTimer::new());
        let service = SyncService::new(
            Arc::<MemoryFileSystem>::clone(&files),
            Arc::<MemoryBlobStore>::clone(store),
            clock,
            Arc::<InstantTimer>::clone(&timer),
            Arc::new(MemorySnapshotStore::new()),
            derive_vault(FIXTURE_MNEMONIC).expect("the fixture mnemonic derives a vault"),
        );
        Self {
            files,
            timer,
            service,
        }
    }

    fn sync(&self) -> Result<SyncReport, Error> {
        block_on(
            self.service
                .sync_now(SyncOptions::default(), &mut |_progress| {}),
        )
    }
}

fn unavailable() -> StoreError {
    StoreError::Unavailable("the store is busy".to_owned())
}

#[test]
fn two_vaults_round_trip_through_a_memory_store() {
    block_on(sync_round_trip(Arc::new(MemoryBlobStore::new())));
}

#[test]
fn transient_store_failures_are_retried_through_the_timer() {
    let store = Arc::new(MemoryBlobStore::new());
    let vault = Vault::new(&store);
    vault.files.insert("a.md", b"body");
    store.fail_next(unavailable());
    store.fail_next(unavailable());

    let report = vault.sync().expect("the sync outlasts transient failures");

    assert_eq!(report.uploaded, 1);
    assert_eq!(
        vault.timer.slept().len(),
        2,
        "each retry waits on the timer"
    );
    assert_eq!(store.len(), 2, "the note and the manifest are stored");
}

#[test]
fn a_permanent_store_failure_is_not_retried() {
    let store = Arc::new(MemoryBlobStore::new());
    let vault = Vault::new(&store);
    store.fail_next(StoreError::PermissionDenied("the bucket".to_owned()));

    assert!(matches!(
        vault.sync(),
        Err(Error::Store(StoreError::PermissionDenied(_)))
    ));
    assert!(vault.timer.slept().is_empty());
}

#[test]
fn a_store_that_stays_unavailable_fails_after_the_last_attempt() {
    let store = Arc::new(MemoryBlobStore::new());
    let vault = Vault::new(&store);
    for _ in 0..100 {
        store.fail_next(unavailable());
    }

    assert!(matches!(
        block_on(vault.service.test_connection()),
        Err(Error::Store(StoreError::Unavailable(_)))
    ));
    let waits = vault.timer.slept().len();
    assert!(
        (1..100).contains(&waits),
        "the connection test retries a bounded number of times: {waits}"
    );
}

#[test]
fn a_note_whose_object_is_missing_is_skipped_and_kept_in_the_manifest() {
    let store = Arc::new(MemoryBlobStore::new());
    let first = Vault::new(&store);
    let second = Vault::new(&store);
    first.files.insert("a.md", b"body");
    first.sync().expect("the first vault uploads");
    for key in store.keys() {
        if !key.ends_with("manifest.enc") {
            block_on(store.delete(&key)).expect("the object is deleted");
        }
    }

    let report = second
        .sync()
        .expect("a missing object does not fail the sync");

    assert_eq!(report.downloaded, 0);
    assert_eq!(second.files.contents("a.md"), None);
    assert_eq!(
        first.sync().expect("the first vault syncs again"),
        SyncReport::default(),
        "the manifest still lists the note, so nothing is re-uploaded or deleted"
    );
}
