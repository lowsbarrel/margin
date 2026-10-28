use margin_kernel::UnixMillis;
use margin_sync::{
    ManifestEntry, SyncActionKind, compute_actions, conflict_copy_name, merge_tombstones,
    prune_tombstones,
};

const NOW: u64 = 2_000_000_000;
const TOMBSTONE_TTL: u64 = 90 * 24 * 60 * 60;

fn entry(path: &str, hash: &str, modified: u64) -> ManifestEntry {
    ManifestEntry {
        path: path.to_owned(),
        hash: hash.to_owned(),
        modified,
        deleted_at: None,
    }
}

fn tombstone(path: &str, hash: &str, modified: u64, deleted_at: u64) -> ManifestEntry {
    ManifestEntry {
        deleted_at: Some(deleted_at),
        ..entry(path, hash, modified)
    }
}

fn kind(
    base: &[ManifestEntry],
    local: &[ManifestEntry],
    remote: &[ManifestEntry],
) -> Option<SyncActionKind> {
    let actions = compute_actions(base, local, remote, NOW);
    actions
        .iter()
        .find(|action| action.path == "x.md")
        .map(|action| action.kind)
}

#[test]
fn a_path_missing_from_the_remote_is_uploaded_not_deleted() {
    let recent = [entry("x.md", "h1", NOW)];
    assert_eq!(kind(&recent, &recent, &[]), Some(SyncActionKind::Upload));
    assert_eq!(
        kind(&recent, &[entry("x.md", "h2", NOW + 1)], &[]),
        Some(SyncActionKind::Upload)
    );
}

#[test]
fn a_pruned_tombstone_deletes_or_conflicts() {
    let old = NOW - TOMBSTONE_TTL - 10;
    let base = [entry("x.md", "h1", old)];
    assert_eq!(kind(&base, &base, &[]), Some(SyncActionKind::DeleteLocal));
    assert_eq!(
        kind(&base, &[entry("x.md", "h2", old + 5)], &[]),
        Some(SyncActionKind::ConflictDeleteRemote)
    );
}

#[test]
fn a_remote_tombstone_deletes_locally_unless_the_note_changed() {
    let base = [entry("x.md", "h1", 1)];
    let remote = [tombstone("x.md", "h1", 1, 5)];
    assert_eq!(
        kind(&base, &base, &remote),
        Some(SyncActionKind::DeleteLocal)
    );
    assert_eq!(
        kind(&base, &[entry("x.md", "h2", 2)], &remote),
        Some(SyncActionKind::ConflictDeleteRemote)
    );
}

#[test]
fn a_local_deletion_deletes_remotely_unless_the_remote_changed() {
    let base = [entry("x.md", "h1", 1)];
    assert_eq!(kind(&base, &[], &base), Some(SyncActionKind::DeleteRemote));
    assert_eq!(
        kind(
            &base,
            &[tombstone("x.md", "h1", 1, 6)],
            &[entry("x.md", "h2", 2)]
        ),
        Some(SyncActionKind::ConflictDeleteLocal)
    );
}

#[test]
fn one_sided_edits_transfer_and_two_sided_edits_conflict() {
    let base = [entry("x.md", "h1", 1)];
    let local = [entry("x.md", "h2", 2)];
    let remote = [entry("x.md", "h3", 3)];
    assert_eq!(kind(&base, &local, &base), Some(SyncActionKind::Upload));
    assert_eq!(kind(&base, &base, &remote), Some(SyncActionKind::Download));
    assert_eq!(kind(&base, &local, &remote), Some(SyncActionKind::Conflict));
    assert_eq!(kind(&[], &local, &remote), Some(SyncActionKind::Conflict));
    assert_eq!(kind(&base, &base, &base), None);
}

#[test]
fn new_notes_upload_or_download() {
    assert_eq!(
        kind(&[], &[entry("x.md", "h1", 1)], &[]),
        Some(SyncActionKind::Upload)
    );
    assert_eq!(
        kind(&[], &[], &[entry("x.md", "h1", 1)]),
        Some(SyncActionKind::Download)
    );
}

#[test]
fn merging_tombstones_keeps_the_latest_deletion_and_pruning_drops_expired_ones() {
    let merged = merge_tombstones(
        &[
            tombstone("x.md", "h1", 1, 10),
            tombstone("y.md", "h1", 1, 10),
        ],
        &[tombstone("x.md", "h2", 2, 20)],
    );
    assert_eq!(
        merged,
        [
            tombstone("x.md", "h2", 2, 20),
            tombstone("y.md", "h1", 1, 10)
        ]
    );

    let kept = tombstone("x.md", "h1", 1, NOW - TOMBSTONE_TTL + 1);
    let expired = tombstone("y.md", "h1", 1, NOW - TOMBSTONE_TTL);
    assert_eq!(prune_tombstones(&[kept.clone(), expired], NOW), [kept]);
}

#[test]
fn a_conflict_copy_keeps_the_extension_of_the_file_name_only() {
    let at = UnixMillis::from_millis(1_700_000_000_000);
    assert_eq!(
        conflict_copy_name("notes/a.md", at, 1),
        "notes/a.sync-conflict-20231114-221320-1.md"
    );
    assert_eq!(
        conflict_copy_name("notes.d/readme", at, 2),
        "notes.d/readme.sync-conflict-20231114-221320-2"
    );
    assert_eq!(
        conflict_copy_name("notes/.env", at, 3),
        "notes/.env.sync-conflict-20231114-221320-3"
    );
}
