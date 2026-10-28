use std::fs;
use std::path::Path;
#[cfg(windows)]
use std::process::{Command, Stdio};

use margin_fs::StdFileSystem;
use margin_kernel::{EntryKind, FileSystem, RelativePath, StoreError};

#[test]
fn a_symlink_out_of_the_root_is_never_followed() {
    let parent = tempfile::tempdir().expect("a temporary directory");
    let root = parent.path().join("vault");
    let outside = parent.path().join("outside");
    fs::create_dir_all(&root).expect("a root");
    fs::create_dir_all(&outside).expect("an outside directory");
    fs::write(outside.join("secret.md"), b"secret").expect("a secret file");
    if !link_directory(&outside, &root.join("escape")) {
        return;
    }
    let filesystem = StdFileSystem::new(root);

    let error = filesystem
        .read(&path("escape/secret.md"))
        .expect_err("reading through the link is refused");
    assert!(
        matches!(error, StoreError::OutsideRoot(_)),
        "unexpected error: {error}"
    );
    let error = filesystem
        .write(&path("escape/written.md"), b"x")
        .expect_err("writing through the link is refused");
    assert!(
        matches!(error, StoreError::OutsideRoot(_)),
        "unexpected error: {error}"
    );
    assert!(!outside.join("written.md").exists());
    assert_eq!(
        fs::read(outside.join("secret.md")).expect("the secret survives"),
        b"secret"
    );

    let entries = filesystem
        .list(&RelativePath::root())
        .expect("the root lists");
    let escape = entries
        .iter()
        .find(|entry| entry.name == "escape")
        .expect("the link is listed");
    assert_eq!(escape.metadata.kind, EntryKind::Symlink);
}

fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("the test path is valid")
}

#[cfg(unix)]
fn link_directory(target: &Path, link: &Path) -> bool {
    use std::os::unix::fs::symlink;
    symlink(target, link).is_ok()
}

#[cfg(windows)]
fn link_directory(target: &Path, link: &Path) -> bool {
    use std::os::windows::fs::symlink_dir;
    if symlink_dir(target, link).is_ok() {
        return true;
    }
    privilege_free_junction(target, link)
}

#[cfg(windows)]
fn privilege_free_junction(target: &Path, link: &Path) -> bool {
    let created = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    created && link.exists()
}
