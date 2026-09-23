use super::scan::{SWEEP_GRACE, is_stored_name, sweep_unused, unused_files};
use super::{MAX_STEM, attachment_name, hash_prefix, import_file, sanitize_stem, store};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "margin-attachments-test-{}-{tag}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_name_is_the_sanitized_stem_plus_the_content_hash() {
    let hash = hash_prefix(b"abc");
    assert_eq!(
        attachment_name("My? Photo.png", b"abc"),
        format!("My_Photo-{hash}.png")
    );
    assert_eq!(
        attachment_name("shot.PNG", b"abc"),
        format!("shot-{hash}.png")
    );
    assert_eq!(
        attachment_name("archive", b"abc"),
        format!("archive-{hash}")
    );
    assert_eq!(attachment_name(".hidden", b"abc"), format!("hidden-{hash}"));
    assert_eq!(attachment_name("a/.b/.png", b"abc"), format!("png-{hash}"));
    assert_eq!(
        attachment_name(&format!("{}.png", "a".repeat(200)), b"abc"),
        format!("{}-{hash}.png", "a".repeat(MAX_STEM))
    );

    assert_eq!(sanitize_stem("../../etc/passwd"), "etc_passwd");
    assert_eq!(sanitize_stem("!!!"), "file");
    assert_eq!(sanitize_stem(".hidden"), "hidden");
}

#[test]
fn the_same_content_reuses_its_file() {
    let dir = temp_dir("dedupe");
    let first = store(&dir, "shot.png", b"same bytes").unwrap();
    let second = store(&dir, "shot.png", b"same bytes").unwrap();

    assert_eq!(first, second, "identical content must resolve to one file");
    assert_eq!(files_in(&dir), vec![first]);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_taken_name_is_never_overwritten() {
    let dir = temp_dir("collision");
    let taken = format!("shot-{}.png", hash_prefix(b"second"));
    fs::write(dir.join(&taken), "the original").unwrap();

    let stored = store(&dir, "shot.png", b"second").unwrap();

    assert_ne!(stored, taken, "a different file must not take the name");
    assert_eq!(stored, format!("shot-{}-2.png", hash_prefix(b"second")));
    assert_eq!(
        fs::read_to_string(dir.join(&taken)).unwrap(),
        "the original",
        "the file that was there must be untouched"
    );
    assert_eq!(files_in(&dir).len(), 2);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn importing_reads_the_source_and_keeps_its_name() {
    let dir = temp_dir("import");
    let src = dir.join("holiday.JPEG");
    fs::write(&src, b"photo").unwrap();
    let dest = dir.join("attachments");

    let rel = import_file(&src, &dest, "attachments").unwrap();

    assert_eq!(
        rel,
        format!("attachments/holiday-{}.jpeg", hash_prefix(b"photo"))
    );
    assert_eq!(
        fs::read(dir.join("attachments").join(&rel["attachments/".len()..])).unwrap(),
        b"photo"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn only_unreferenced_attachments_are_reported() {
    let root = temp_dir("unused");
    let notes = root.join("notes");
    let att = root.join("attachments");
    fs::create_dir_all(&notes).unwrap();
    fs::create_dir_all(&att).unwrap();
    fs::write(root.join("index.md"), "![a](attachments/used.png)").unwrap();
    fs::write(notes.join("b.md"), "![[embedded.pdf]]").unwrap();
    fs::write(
        notes.join("c.md"),
        "<img src=\"attachments/space%20name.png\">\n[spaced](<attachments/angle.png>)",
    )
    .unwrap();
    for name in [
        "used.png",
        "embedded.pdf",
        "space name.png",
        "angle.png",
        "orphan.png",
    ] {
        fs::write(att.join(name), b"x").unwrap();
    }

    let unused = unused_files(&root.to_string_lossy(), "attachments").unwrap();

    assert_eq!(unused, vec!["attachments/orphan.png".to_string()]);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_hidden_note_does_not_keep_a_file() {
    let root = temp_dir("unused-hidden");
    let att = root.join("attachments");
    fs::create_dir_all(root.join(".margin")).unwrap();
    fs::create_dir_all(&att).unwrap();
    fs::write(
        root.join(".margin/backup.md"),
        "![a](attachments/orphan.png)",
    )
    .unwrap();
    fs::write(att.join("orphan.png"), b"x").unwrap();

    let unused = unused_files(&root.to_string_lossy(), "attachments").unwrap();

    assert_eq!(unused, vec!["attachments/orphan.png".to_string()]);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_sweep_trashes_only_old_unreferenced_files_the_app_stored() {
    let root = temp_dir("sweep");
    let att = root.join("attachments");
    fs::create_dir_all(&att).unwrap();
    let hash = hash_prefix(b"x");
    let used = format!("used-{hash}.png");
    let stale = format!("stale-{hash}.png");
    let clash = format!("clash-{hash}-2.png");
    let fresh = format!("fresh-{hash}.png");
    let by_hand = "hand-placed.pdf".to_string();
    fs::write(root.join("note.md"), format!("![a](attachments/{used})")).unwrap();
    for name in [&used, &stale, &clash, &fresh, &by_hand] {
        fs::write(att.join(name), b"x").unwrap();
    }
    let month_ago = SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
    let month_ago = filetime::FileTime::from_system_time(month_ago);
    for name in [&used, &stale, &clash, &by_hand] {
        filetime::set_file_mtime(att.join(name), month_ago).unwrap();
    }

    let swept = sweep_unused(&root.to_string_lossy(), "attachments", SWEEP_GRACE).unwrap();

    assert_eq!(swept, 2);
    let mut kept = vec![by_hand, fresh, used];
    kept.sort();
    assert_eq!(files_in(&att), kept);
    assert!(root.join(".margin/trash").is_dir());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn stored_names_are_recognised_by_their_hash_tail() {
    assert!(is_stored_name("shot-0a1b2c3d.png"));
    assert!(is_stored_name("shot-0a1b2c3d-2.png"));
    assert!(is_stored_name("noext-12345678"));
    assert!(!is_stored_name("shot.png"));
    assert!(!is_stored_name("shot-0A1B2C3D.png"));
    assert!(!is_stored_name("shot-0a1b2c3.png"));
    assert!(!is_stored_name("-0a1b2c3d.png"));
}
