use super::*;
use std::fs;
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "margin-fs-test-{}-{tag}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn copying_a_directory_onto_an_existing_one_is_refused() {
    let dir = temp_dir("copy-dir");
    let src = dir.join("src");
    fs::create_dir_all(src.join("sub")).unwrap();
    fs::write(src.join("sub").join("a.md"), "a").unwrap();
    let occupied = dir.join("occupied");
    fs::create_dir_all(&occupied).unwrap();
    fs::write(occupied.join("keep.md"), "keep").unwrap();

    assert!(copy_dir_recursive(&src, &occupied).is_err());
    assert_eq!(
        fs::read_to_string(occupied.join("keep.md")).unwrap(),
        "keep"
    );

    let free = dir.join("free");
    copy_dir_recursive(&src, &free).unwrap();
    assert_eq!(
        fs::read_to_string(free.join("sub").join("a.md")).unwrap(),
        "a"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn importing_an_external_directory_copies_its_contents() {
    let external = temp_dir("import-dir-source");
    fs::create_dir_all(external.join("nested")).unwrap();
    fs::write(external.join("note.md"), "one").unwrap();
    fs::write(external.join("nested").join("deep.md"), "two").unwrap();

    let vault = temp_dir("import-dir-vault");
    let dst = vault.join("imported");

    import_dir_at(&external, &dst).unwrap();

    assert_eq!(fs::read_to_string(dst.join("note.md")).unwrap(), "one");
    assert_eq!(
        fs::read_to_string(dst.join("nested").join("deep.md")).unwrap(),
        "two"
    );
    assert_eq!(fs::read_to_string(external.join("note.md")).unwrap(), "one");

    fs::remove_dir_all(&external).ok();
    fs::remove_dir_all(&vault).ok();
}

#[test]
fn importing_a_directory_onto_an_existing_one_is_refused() {
    let external = temp_dir("import-dir-conflict-source");
    fs::write(external.join("a.md"), "a").unwrap();
    let vault = temp_dir("import-dir-conflict-vault");
    let dst = vault.join("occupied");
    fs::create_dir_all(&dst).unwrap();
    fs::write(dst.join("keep.md"), "keep").unwrap();

    assert!(import_dir_at(&external, &dst).is_err());
    assert_eq!(fs::read_to_string(dst.join("keep.md")).unwrap(), "keep");
    assert!(!dst.join("a.md").exists());

    fs::remove_dir_all(&external).ok();
    fs::remove_dir_all(&vault).ok();
}

#[test]
fn importing_a_file_as_a_directory_is_refused() {
    let external = temp_dir("import-dir-file-source");
    let file = external.join("note.md");
    fs::write(&file, "body").unwrap();
    let vault = temp_dir("import-dir-file-vault");
    let dst = vault.join("note.md");

    assert!(import_dir_at(&file, &dst).is_err());
    assert!(!dst.exists());

    fs::remove_dir_all(&external).ok();
    fs::remove_dir_all(&vault).ok();
}

#[test]
fn importing_a_directory_into_itself_is_refused() {
    let external = temp_dir("import-dir-self");
    fs::create_dir_all(external.join("sub")).unwrap();

    assert!(import_dir_at(&external, &external.join("sub")).is_err());
    assert!(!external.join("sub").join("sub").exists());

    fs::remove_dir_all(&external).ok();
}

#[cfg(unix)]
#[test]
fn importing_a_directory_does_not_follow_symlinks() {
    let outside = temp_dir("import-dir-outside");
    fs::write(outside.join("secret.md"), "secret").unwrap();
    let external = temp_dir("import-dir-link-source");
    fs::write(external.join("real.md"), "real").unwrap();
    std::os::unix::fs::symlink(&outside, external.join("link")).unwrap();

    let vault = temp_dir("import-dir-link-vault");
    let dst = vault.join("imported");
    import_dir_at(&external, &dst).unwrap();

    assert_eq!(fs::read_to_string(dst.join("real.md")).unwrap(), "real");
    assert!(
        !dst.join("link").exists(),
        "a symlinked directory must not be copied into the vault"
    );

    fs::remove_dir_all(&outside).ok();
    fs::remove_dir_all(&external).ok();
    fs::remove_dir_all(&vault).ok();
}
