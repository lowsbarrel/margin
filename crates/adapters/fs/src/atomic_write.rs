use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use margin_kernel::StoreError;

use crate::io_error::from_io;

static TEMPORARY_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Privacy {
    Inherited,
    OwnerOnly,
}

impl Privacy {
    fn is_owner_only(self) -> bool {
        matches!(self, Self::OwnerOnly)
    }
}

pub(crate) fn atomic_write(
    destination: &Path,
    contents: &[u8],
    privacy: Privacy,
) -> Result<(), StoreError> {
    #[cfg(windows)]
    if privacy.is_owner_only() && destination.exists() {
        set_read_only(destination, false)?;
    }
    let temporary = destination.with_file_name(format!(
        ".margin-write-{}.tmp",
        TEMPORARY_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    if let Err(error) = write_temporary_file(&temporary, destination, contents, privacy) {
        let _removed = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, destination) {
        let _removed = fs::remove_file(&temporary);
        return Err(from_io(&error, destination));
    }
    #[cfg(windows)]
    if privacy.is_owner_only() {
        set_read_only(destination, true)?;
    }
    if let Some(parent) = destination.parent()
        && let Ok(directory) = fs::File::open(parent)
    {
        let _synced = directory.sync_all();
    }
    Ok(())
}

fn write_temporary_file(
    temporary: &Path,
    destination: &Path,
    contents: &[u8],
    privacy: Privacy,
) -> Result<(), StoreError> {
    let mut file = create_temporary_file(temporary, privacy)?;
    match privacy {
        Privacy::Inherited => {
            if let Ok(existing) = fs::metadata(destination) {
                let _permissions_kept_across_the_rename =
                    file.set_permissions(existing.permissions());
            }
        }
        Privacy::OwnerOnly => {
            #[cfg(unix)]
            owner_only_permissions(temporary)?;
        }
    }
    file.write_all(contents)
        .map_err(|error| from_io(&error, temporary))?;
    file.sync_all()
        .map_err(|error| from_io(&error, temporary))?;
    Ok(())
}

fn create_temporary_file(temporary: &Path, privacy: Privacy) -> Result<fs::File, StoreError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    apply_owner_only_create_mode(&mut options, privacy);
    options
        .open(temporary)
        .map_err(|error| from_io(&error, temporary))
}

#[cfg(unix)]
fn apply_owner_only_create_mode(options: &mut fs::OpenOptions, privacy: Privacy) {
    use std::os::unix::fs::OpenOptionsExt;
    if privacy.is_owner_only() {
        options.mode(0o600);
    }
}

#[cfg(not(unix))]
fn apply_owner_only_create_mode(_options: &mut fs::OpenOptions, _privacy: Privacy) {}

#[cfg(unix)]
fn owner_only_permissions(path: &Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| from_io(&error, path))
}

#[cfg(windows)]
fn set_read_only(path: &Path, read_only: bool) -> Result<(), StoreError> {
    let mut permissions = fs::metadata(path)
        .map_err(|error| from_io(&error, path))?
        .permissions();
    permissions.set_readonly(read_only);
    fs::set_permissions(path, permissions).map_err(|error| from_io(&error, path))
}
