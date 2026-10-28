use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use margin_attachments::{AttachmentsService, Error, SWEEP_GRACE};
use margin_kernel::{FileSystem, HistoryStore, RelativePath, UnixMillis};
use margin_test_support::{FixedClock, MemoryFileSystem, MemoryHistoryStore, MemoryHostFiles};
use margin_trash::TrashService;
use sha2::{Digest, Sha256};

const NOW: u64 = 1_700_000_000_123;
const FOLDER: &str = "attachments";
const MONTH: Duration = Duration::from_hours(30 * 24);

struct Fixture {
    vault: Arc<MemoryFileSystem>,
    host: Arc<MemoryHostFiles>,
    attachments: AttachmentsService,
}

impl Fixture {
    fn new() -> Self {
        let clock = Arc::new(FixedClock::new(UnixMillis::from_millis(NOW)));
        let vault = Arc::new(MemoryFileSystem::with_clock(Arc::clone(&clock)));
        let host = Arc::new(MemoryHostFiles::new());
        let history: Arc<dyn HistoryStore> = Arc::new(MemoryHistoryStore::new(Arc::clone(&vault)));
        let trash = Arc::new(TrashService::new(
            Arc::<MemoryFileSystem>::clone(&vault),
            Arc::clone(&history),
            Arc::<FixedClock>::clone(&clock),
        ));
        let attachments = AttachmentsService::new(
            Arc::<MemoryFileSystem>::clone(&vault),
            Arc::<MemoryHostFiles>::clone(&host),
            trash,
            clock,
        );
        Self {
            vault,
            host,
            attachments,
        }
    }

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("a test path is valid")
    }

    fn folder() -> RelativePath {
        Self::path(FOLDER)
    }

    fn write(&self, path: &str, contents: &str) {
        self.vault.insert(path, contents.as_bytes());
    }

    fn names(&self, folder: &str) -> Vec<String> {
        let mut names: Vec<String> = self
            .vault
            .list(&Self::path(folder))
            .map(|entries| entries.into_iter().map(|entry| entry.name).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    fn backdate(&self, path: &str) {
        let stamp = UnixMillis::from_millis(NOW).saturating_sub(MONTH);
        self.vault
            .set_modified(&Self::path(path), stamp)
            .expect("the file exists");
    }
}

fn hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let encoded = hex::encode(digest);
    encoded[..8].to_owned()
}

#[test]
fn the_name_is_the_sanitized_stem_plus_the_content_hash() {
    let fixture = Fixture::new();
    let digest = hash(b"abc");

    let cases = [
        ("My? Photo.png", format!("My_Photo-{digest}.png")),
        ("shot.PNG", format!("shot-{digest}.png")),
        ("archive", format!("archive-{digest}")),
        (".hidden", format!("hidden-{digest}")),
        ("a/.b/.png", format!("png-{digest}")),
        ("../../etc/passwd", format!("passwd-{digest}")),
        ("!!!", format!("file-{digest}")),
        (
            &format!("{}.png", "a".repeat(200)),
            format!("{}-{digest}.png", "a".repeat(64)),
        ),
    ];
    for (original, expected) in cases {
        let stored = fixture
            .attachments
            .store(&Fixture::folder(), original, b"abc")
            .expect("the attachment is stored");
        assert_eq!(
            stored.as_str(),
            format!("{FOLDER}/{expected}"),
            "for {original}"
        );
        fixture
            .vault
            .remove_dir_all(&Fixture::folder())
            .expect("the folder exists");
    }
}

#[test]
fn the_same_content_reuses_its_file() {
    let fixture = Fixture::new();
    let first = fixture
        .attachments
        .store(&Fixture::folder(), "shot.png", b"same bytes")
        .expect("the attachment is stored");
    let second = fixture
        .attachments
        .store(&Fixture::folder(), "shot.png", b"same bytes")
        .expect("the attachment is stored again");

    assert_eq!(first, second, "identical content must resolve to one file");
    assert_eq!(fixture.names(FOLDER), vec![first.file_name().to_owned()]);
}

#[test]
fn a_taken_name_is_never_overwritten() {
    let fixture = Fixture::new();
    let taken = format!("shot-{}.png", hash(b"second"));
    fixture.write(&format!("{FOLDER}/{taken}"), "the original");

    let stored = fixture
        .attachments
        .store(&Fixture::folder(), "shot.png", b"second")
        .expect("the attachment is stored");

    assert_ne!(stored.file_name(), taken);
    assert_eq!(
        stored.as_str(),
        format!("{FOLDER}/shot-{}-2.png", hash(b"second"))
    );
    assert_eq!(
        fixture.vault.contents(&format!("{FOLDER}/{taken}")),
        Some(b"the original".to_vec()),
        "the file that was there must be untouched"
    );
    assert_eq!(fixture.names(FOLDER).len(), 2);
}

#[test]
fn importing_reads_the_source_and_keeps_its_name() {
    let fixture = Fixture::new();
    fixture
        .host
        .insert(Path::new("/drop/holiday.JPEG"), b"photo");

    let stored = fixture
        .attachments
        .import_file(Path::new("/drop/holiday.JPEG"), &Fixture::folder())
        .expect("the dropped file is imported");

    assert_eq!(
        stored.as_str(),
        format!("{FOLDER}/holiday-{}.jpeg", hash(b"photo"))
    );
    assert_eq!(
        fixture.vault.contents(stored.as_str()),
        Some(b"photo".to_vec())
    );
}

#[test]
fn a_folder_that_is_hidden_or_the_root_is_refused() {
    let fixture = Fixture::new();

    assert!(matches!(
        fixture
            .attachments
            .store(&RelativePath::root(), "a.png", b"x"),
        Err(Error::InvalidFolder(_))
    ));
    assert!(matches!(
        fixture
            .attachments
            .store(&Fixture::path(".hidden"), "a.png", b"x"),
        Err(Error::InvalidFolder(_))
    ));
    assert!(matches!(
        fixture.attachments.unused_files(&Fixture::path(".hidden")),
        Err(Error::InvalidFolder(_))
    ));
}

#[test]
fn only_unreferenced_attachments_are_reported() {
    let fixture = Fixture::new();
    fixture.write("index.md", "![a](attachments/used.png)");
    fixture.write("notes/b.md", "![[embedded.pdf]]");
    fixture.write(
        "notes/c.md",
        "<img src=\"attachments/space%20name.png\">\n[spaced](<attachments/angle.png>)",
    );
    for name in [
        "used.png",
        "embedded.pdf",
        "space name.png",
        "angle.png",
        "orphan.png",
    ] {
        fixture.write(&format!("{FOLDER}/{name}"), "x");
    }

    let unused: Vec<String> = fixture
        .attachments
        .unused_files(&Fixture::folder())
        .expect("the folder is scanned")
        .into_iter()
        .map(|path| path.to_string())
        .collect();

    assert_eq!(unused, vec![String::from("attachments/orphan.png")]);
}

#[test]
fn a_hidden_note_does_not_keep_a_file() {
    let fixture = Fixture::new();
    fixture.write(".margin/backup.md", "![a](attachments/orphan.png)");
    fixture.write(&format!("{FOLDER}/orphan.png"), "x");

    let unused = fixture
        .attachments
        .unused_files(&Fixture::folder())
        .expect("the folder is scanned");

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].as_str(), "attachments/orphan.png");
}

#[test]
fn an_absent_folder_has_nothing_unused() {
    let fixture = Fixture::new();
    fixture.write("note.md", "no attachments here");

    assert!(
        fixture
            .attachments
            .unused_files(&Fixture::folder())
            .expect("the scan succeeds")
            .is_empty()
    );
}

#[test]
fn the_sweep_trashes_only_old_unreferenced_files_the_app_stored() {
    let fixture = Fixture::new();
    let digest = hash(b"x");
    let used = format!("used-{digest}.png");
    let stale = format!("stale-{digest}.png");
    let clash = format!("clash-{digest}-2.png");
    let fresh = format!("fresh-{digest}.png");
    let by_hand = String::from("hand-placed.pdf");
    fixture.write("note.md", &format!("![a](attachments/{used})"));
    for name in [&used, &stale, &clash, &fresh, &by_hand] {
        fixture.write(&format!("{FOLDER}/{name}"), "x");
    }
    for name in [&used, &stale, &clash, &by_hand] {
        fixture.backdate(&format!("{FOLDER}/{name}"));
    }

    let swept = fixture
        .attachments
        .sweep_unused(&Fixture::folder(), SWEEP_GRACE)
        .expect("the sweep runs");

    assert_eq!(swept, 2);
    let mut kept = vec![by_hand, fresh, used];
    kept.sort();
    assert_eq!(fixture.names(FOLDER), kept);
    assert!(fixture.vault.exists(&Fixture::path(".margin/trash")));
}

#[test]
fn the_sweep_runs_without_any_notes() {
    let fixture = Fixture::new();
    fixture.write(&format!("{FOLDER}/orphan-{}.png", hash(b"x")), "x");
    fixture.backdate(&format!("{FOLDER}/orphan-{}.png", hash(b"x")));

    assert_eq!(
        fixture
            .attachments
            .sweep_unused(&Fixture::folder(), SWEEP_GRACE)
            .expect("the sweep runs"),
        1
    );
}
