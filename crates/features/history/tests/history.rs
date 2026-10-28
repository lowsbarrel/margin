use std::sync::Arc;
use std::time::Duration;

use margin_history::{Error, History};
use margin_kernel::{RelativePath, StoreError, UnixMillis};
use margin_test_support::{FixedClock, MemoryFileSystem};

const NOW: u64 = 1_700_000_000_000;
const HOUR_MS: u64 = 60 * 60 * 1000;
const DAY_MS: u64 = 24 * HOUR_MS;
const WEEK_MS: u64 = 7 * DAY_MS;

fn setup(now: u64) -> (Arc<MemoryFileSystem>, Arc<FixedClock>, History) {
    let clock = Arc::new(FixedClock::new(UnixMillis::from_millis(now)));
    let files = Arc::new(MemoryFileSystem::with_clock(Arc::clone(&clock)));
    let history = History::new(
        Arc::<MemoryFileSystem>::clone(&files),
        Arc::<FixedClock>::clone(&clock),
    );
    (files, clock, history)
}

fn note(path: &str) -> RelativePath {
    RelativePath::new(path).expect("test paths are valid relative paths")
}

#[test]
fn identical_bytes_do_not_pile_up() {
    let (_files, _clock, history) = setup(NOW);
    let note = note("note.md");

    let first = history.save(&note, b"previous").expect("the call succeeds");
    let second = history.save(&note, b"previous").expect("the call succeeds");
    let third = history.save(&note, b"previous").expect("the call succeeds");

    assert_eq!(first, second);
    assert_eq!(second, third);
    assert_eq!(history.list(&note).expect("the call succeeds").len(), 1);

    history.save(&note, b"other").expect("the call succeeds");
    assert_eq!(history.list(&note).expect("the call succeeds").len(), 2);
}

#[test]
fn a_snapshot_in_the_same_millisecond_gets_its_own_file() {
    let (_files, _clock, history) = setup(NOW);
    let note = note("note.md");

    history.save(&note, b"one").expect("the call succeeds");
    history.save(&note, b"two").expect("the call succeeds");

    let listed = history.list(&note).expect("the call succeeds");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].filename, format!("{}.md", NOW + 1));
    assert_eq!(listed[1].filename, format!("{NOW}.md"));
}

#[test]
fn list_reports_seconds_timestamp_and_size_newest_first() {
    let (_files, clock, history) = setup(NOW);
    let note = note("notes/todo.md");

    history.save(&note, b"one").expect("the call succeeds");
    clock.advance(Duration::from_millis(1000));
    history.save(&note, b"two").expect("the call succeeds");

    let listed = history.list(&note).expect("the call succeeds");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].filename, format!("{}.md", NOW + 1000));
    assert_eq!(listed[0].timestamp, (NOW + 1000) / 1000);
    assert_eq!(listed[0].size, 3);
    assert!(listed[0].timestamp > listed[1].timestamp);
}

#[test]
fn list_of_a_note_without_history_is_empty() {
    let (_files, _clock, history) = setup(NOW);
    assert!(
        history
            .list(&note("note.md"))
            .expect("the call succeeds")
            .is_empty()
    );
}

#[test]
fn read_returns_the_saved_bytes_and_delete_removes_them() {
    let (_files, _clock, history) = setup(NOW);
    let note = note("note.md");

    let name = history.save(&note, b"content").expect("the call succeeds");
    assert_eq!(
        history.read(&note, &name).expect("the call succeeds"),
        b"content".to_vec()
    );

    history.delete(&note, &name).expect("the call succeeds");
    assert!(matches!(
        history.read(&note, &name),
        Err(Error::Store(StoreError::NotFound(_)))
    ));
}

#[test]
fn rejects_snapshot_names_that_escape_the_history_directory() {
    let (_files, _clock, history) = setup(NOW);
    let note = note("note.md");

    for escaping in ["../secret.md", "a/b.md", "c\\d.md"] {
        assert!(matches!(
            history.read(&note, escaping),
            Err(Error::InvalidSnapshotFilename(_))
        ));
    }
    assert!(matches!(
        history.delete(&note, ".."),
        Err(Error::InvalidSnapshotFilename(_))
    ));
}

#[test]
fn clear_removes_every_snapshot_and_counts_them() {
    let (_files, _clock, history) = setup(NOW);
    let note = note("note.md");

    history.save(&note, b"one").expect("the call succeeds");
    history.save(&note, b"two").expect("the call succeeds");

    assert_eq!(history.clear(&note).expect("the call succeeds"), 2);
    assert!(history.list(&note).expect("the call succeeds").is_empty());
    assert_eq!(history.clear(&note).expect("the call succeeds"), 0);
}

#[test]
fn rename_moves_a_note_history() {
    let (_files, _clock, history) = setup(NOW);
    let old = note("old.md");
    let new = note("new.md");

    let name = history.save(&old, b"kept").expect("the call succeeds");
    history.rename(&old, &new).expect("the call succeeds");

    assert!(history.list(&old).expect("the call succeeds").is_empty());
    assert_eq!(
        history.read(&new, &name).expect("the call succeeds"),
        b"kept".to_vec()
    );
}

#[test]
fn rename_of_a_note_without_history_is_a_no_op() {
    let (_files, _clock, history) = setup(NOW);
    history
        .rename(&note("missing.md"), &note("other.md"))
        .expect("the call succeeds");
}

#[test]
fn case_only_rename_keeps_every_snapshot() {
    let (files, _clock, history) = setup(NOW);
    files.insert(".margin/history/Note.md/1712928000000.md", b"one");
    files.insert(".margin/history/Note.md/1712928000001.md", b"two");

    history
        .rename(&note("Note.md"), &note("note.md"))
        .expect("the call succeeds");

    assert_eq!(
        history
            .list(&note("note.md"))
            .expect("the call succeeds")
            .len(),
        2
    );
    assert_eq!(
        files
            .contents(".margin/history/note.md/1712928000000.md")
            .expect("the call succeeds"),
        b"one".to_vec()
    );
}

#[test]
fn merging_keeps_both_sets_and_never_overwrites() {
    let (files, _clock, history) = setup(NOW);
    files.insert(".margin/history/old/note.md/1712928000000.md", b"from old");
    files.insert(
        ".margin/history/old/note.md/1712928000001.md",
        b"only old had this",
    );
    files.insert(".margin/history/new/note.md/1712928000000.md", b"from new");

    history
        .rename(&note("old/note.md"), &note("new/note.md"))
        .expect("the call succeeds");

    assert_eq!(
        history
            .list(&note("new/note.md"))
            .expect("the call succeeds")
            .len(),
        3
    );
    assert_eq!(
        files
            .contents(".margin/history/new/note.md/1712928000000.md")
            .expect("the call succeeds"),
        b"from new".to_vec()
    );
    assert_eq!(
        files
            .contents(".margin/history/new/note.md/1712928000002.md")
            .expect("the call succeeds"),
        b"only old had this".to_vec()
    );
    assert!(
        files
            .contents(".margin/history/old/note.md/1712928000000.md")
            .is_none()
    );
}

#[test]
fn merging_recurses_into_subdirectories() {
    let (files, _clock, history) = setup(NOW);
    files.insert(".margin/history/old/sub/a.md/1.md", b"a");
    files.insert(".margin/history/new/sub/a.md/2.md", b"b");

    history
        .rename(&note("old"), &note("new"))
        .expect("the call succeeds");

    assert_eq!(
        history
            .list(&note("new/sub/a.md"))
            .expect("the call succeeds")
            .len(),
        2
    );
    assert!(
        files
            .contents(".margin/history/old/sub/a.md/1.md")
            .is_none()
    );
}

#[test]
fn merging_drops_identical_snapshots() {
    let (files, _clock, history) = setup(NOW);
    files.insert(".margin/history/old/note.md/1712928000000.md", b"same");
    files.insert(".margin/history/new/note.md/1712928000000.md", b"same");

    history
        .rename(&note("old/note.md"), &note("new/note.md"))
        .expect("the call succeeds");

    assert_eq!(
        history
            .list(&note("new/note.md"))
            .expect("the call succeeds")
            .len(),
        1
    );
}

#[test]
fn orphan_history_is_swept_and_live_history_is_kept() {
    let (files, _clock, history) = setup(NOW);
    files.insert(".margin/history/notes/todo.md/1712928000123.md", b"x");
    files.insert(".margin/history/notes/gone.md/1712928000123.md", b"x");
    files.insert("notes/todo.md", b"live");

    assert_eq!(history.prune_orphans(), 1);

    assert!(
        files
            .contents(".margin/history/notes/todo.md/1712928000123.md")
            .is_some()
    );
    assert!(
        files
            .contents(".margin/history/notes/gone.md/1712928000123.md")
            .is_none()
    );
}

#[test]
fn save_prunes_snapshots_retention_drops() {
    let now = 1000 * WEEK_MS;
    let (files, _clock, history) = setup(now);
    let note = note("note.md");

    let kept = [now - 2_000, now - 1_000, now - 2 * DAY_MS + 2 * HOUR_MS];
    let dropped = [now - 2 * DAY_MS + HOUR_MS];
    for stamp in kept.iter().chain(dropped.iter()) {
        files.insert(&format!(".margin/history/note.md/{stamp}.md"), b"old");
    }

    history.save(&note, b"new").expect("the call succeeds");

    for stamp in dropped {
        assert!(
            files
                .contents(&format!(".margin/history/note.md/{stamp}.md"))
                .is_none()
        );
    }
    for stamp in kept {
        assert!(
            files
                .contents(&format!(".margin/history/note.md/{stamp}.md"))
                .is_some()
        );
    }
}
