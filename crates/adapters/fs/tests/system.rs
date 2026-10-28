use std::ffi::OsStr;
use std::fs;

use margin_fs::{app_data_dir, reveal_in_file_manager, write_private_file};

#[test]
fn app_data_dir_ends_with_the_application_identifier() {
    let directory = app_data_dir().expect("the platform has a data directory");
    assert!(
        directory.is_absolute(),
        "app data is absolute: {}",
        directory.display()
    );
    assert_eq!(
        directory.file_name().and_then(OsStr::to_str),
        Some("com.lowsbarrel.margin")
    );
}

#[test]
fn private_files_are_readable_only_by_the_owner() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let key = directory.path().join("nested/device.key");
    write_private_file(&key, b"secret").expect("writing a private file");
    assert_eq!(fs::read(&key).expect("reading the key"), b"secret");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&key)
            .expect("key metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "the key is owner-only: {mode:o}");
    }
    #[cfg(windows)]
    {
        assert!(
            fs::metadata(&key)
                .expect("key metadata")
                .permissions()
                .readonly()
        );
    }
    write_private_file(&key, b"replaced").expect("rewriting a private file");
    assert_eq!(fs::read(&key).expect("reading the key"), b"replaced");
}

#[test]
fn revealing_a_missing_path_fails() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let error = reveal_in_file_manager(&directory.path().join("absent.md"))
        .expect_err("a missing path is refused");
    assert!(
        error.to_string().contains("not found"),
        "unexpected error: {error}"
    );
}
