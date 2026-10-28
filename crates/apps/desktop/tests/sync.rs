use std::fs;
use std::path::Path;

use margin_crypto::derive_vault;
use margin_desktop::{OpenedVault, app_services, open_vault};
use margin_kernel::{RelativePath, UnixMillis};
use margin_settings::{AppSettings, S3Config};
use margin_sync::{SyncOptions, SyncReport};
use margin_test_support::S3Stub;
use margin_workbench::VaultServices;
use tempfile::TempDir;
use tokio::runtime::{Builder, Runtime};

const MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const NOTE: &str = "notes/plan.md";
const LATER: u64 = 2_000_000_000_000;

struct Device {
    root: TempDir,
    _data: TempDir,
    opened: OpenedVault,
}

impl Device {
    fn connect(stub: &S3Stub) -> Self {
        let data = TempDir::new().expect("the data directory is created");
        let root = TempDir::new().expect("the vault directory is created");
        let app = app_services(data.path().to_path_buf());
        let vault = derive_vault(MNEMONIC).expect("the mnemonic is valid");
        let first =
            open_vault(&app, "Notes".to_owned(), root.path(), &vault).expect("the vault opens");
        let settings = AppSettings {
            s3: Some(S3Config {
                endpoint: stub.endpoint(),
                bucket: stub.bucket().to_owned(),
                region: stub.region().to_owned(),
                access_key: stub.access_key().to_owned(),
                secret_key: stub.secret_key().to_owned(),
            }),
            ..AppSettings::default()
        };
        first
            .services
            .settings
            .save(&first.services.key, &settings)
            .expect("the sync settings are saved");
        drop(first);
        let opened = open_vault(&app, "Notes".to_owned(), root.path(), &vault)
            .expect("the vault reopens with sync configured");
        Self {
            root,
            _data: data,
            opened,
        }
    }

    fn services(&self) -> &VaultServices {
        &self.opened.services
    }

    fn sync(&self, runtime: &Runtime) -> SyncReport {
        let service = self
            .services()
            .sync_service()
            .expect("the saved settings configure sync");
        runtime
            .block_on(service.sync_now(SyncOptions::default(), &mut |_| {}))
            .expect("sync succeeds against the stub")
    }

    fn edit(&self, path: &str, text: &str, modified: u64) {
        let path = RelativePath::new(path).expect("the path is valid");
        let files = &self.services().files;
        if let Some(parent) = path.parent() {
            files.create_directory(&parent).expect("the folder exists");
        }
        files
            .write_file(&path, text.as_bytes())
            .expect("the note is written");
        files
            .set_modified(&path, UnixMillis::from_millis(modified))
            .expect("the modification time is set");
    }

    fn text(&self, path: &str) -> String {
        fs::read_to_string(self.root.path().join(path)).expect("the note exists")
    }

    fn conflict_copies(&self) -> Vec<String> {
        visible_files(self.root.path())
            .into_iter()
            .filter(|path| path.contains(".sync-conflict-"))
            .collect()
    }
}

fn visible_files(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("the directory lists") {
            let path = entry.expect("the entry reads").path();
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("the entry is inside the root");
                found.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    found
}

fn runtime() -> Runtime {
    Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("the test runtime starts")
}

#[test]
fn two_devices_with_the_same_mnemonic_converge_and_keep_a_conflict_copy() {
    let stub = S3Stub::start();
    let runtime = runtime();
    let laptop = Device::connect(&stub);
    let desktop = Device::connect(&stub);

    laptop.edit(NOTE, "one", LATER);
    assert_eq!(laptop.sync(&runtime).uploaded, 1);
    assert_eq!(desktop.sync(&runtime).downloaded, 1);
    assert_eq!(desktop.text(NOTE), "one");
    assert_eq!(desktop.sync(&runtime), SyncReport::default());

    laptop.edit(NOTE, "two", LATER + 1_000);
    assert_eq!(laptop.sync(&runtime).uploaded, 1);
    desktop.edit(NOTE, "three", LATER + 2_000);
    let report = desktop.sync(&runtime);
    assert_eq!(report.conflicts, 1, "both devices changed the note");
    assert_eq!(desktop.text(NOTE), "three", "the local body wins");
    let [copy]: [String; 1] = desktop
        .conflict_copies()
        .try_into()
        .expect("one conflict copy is written");
    assert!(copy.starts_with("notes/plan.sync-conflict-"), "{copy}");
    assert_eq!(desktop.text(&copy), "two", "the copy keeps the remote body");

    assert_eq!(laptop.sync(&runtime).downloaded, 2);
    assert_eq!(laptop.text(NOTE), "three");
    assert_eq!(laptop.text(&copy), "two");
    let note = RelativePath::new(NOTE).expect("the path is valid");
    let snapshots = laptop
        .services()
        .history
        .list(&note)
        .expect("the history reads");
    let [snapshot] = snapshots.as_slice() else {
        panic!("the overwritten body is kept in history: {snapshots:?}");
    };
    assert_eq!(
        laptop
            .services()
            .history
            .read(&note, &snapshot.filename)
            .expect("the snapshot reads"),
        b"two"
    );
    assert_eq!(
        visible_files(laptop.root.path()),
        visible_files(desktop.root.path())
    );
}
