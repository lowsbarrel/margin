use super::retention::{
    DAY_MS, HOUR_MS, RETENTION_HARD_CAP, WEEK_MS, prune_snapshots_at, retained_indices,
};
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
    let now = 1000 * WEEK_MS;
    let stamps = vec![
        now - 1_000,
        now - 59 * 60 * 1000,
        now - 2 * HOUR_MS + 20 * 60_000,
        now - 2 * HOUR_MS + 10 * 60_000,
        now - 2 * DAY_MS + 2 * HOUR_MS,
        now - 2 * DAY_MS + HOUR_MS,
        now - 40 * DAY_MS + 2 * HOUR_MS,
        now - 40 * DAY_MS + HOUR_MS,
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
    let stamps: Vec<u64> = (0..3 * 3600u64).map(|s| now - s * 1000).collect();

    let kept = retained_indices(&stamps, now);
    assert_eq!(kept.len(), RETENTION_HARD_CAP);

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
