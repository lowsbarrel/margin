use std::io;

#[cfg(target_os = "macos")]
pub(crate) fn install(bytes: &[u8]) -> io::Result<()> {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::{env, fs};

    use flate2::read::GzDecoder;
    use tar::Archive;

    fn unpack(bytes: &[u8], into: &Path) -> io::Result<PathBuf> {
        Archive::new(GzDecoder::new(bytes)).unpack(into)?;
        fs::read_dir(into)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|extension| extension == "app"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no .app in the update"))
    }

    let executable = env::current_exe()?;
    let bundle = executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "not running from an .app"))?
        .to_path_buf();
    let parent = bundle
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the .app has no parent"))?;
    match tempfile::Builder::new()
        .prefix(".margin-update")
        .tempdir_in(parent)
    {
        Ok(staging) => {
            let fresh = unpack(bytes, staging.path())?;
            let previous = staging.path().join("previous.app");
            fs::rename(&bundle, &previous)?;
            fs::rename(&fresh, &bundle).or_else(|error| {
                fs::rename(&previous, &bundle)?;
                Err(error)
            })
        }
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            let staging = tempfile::tempdir()?;
            let fresh = unpack(bytes, staging.path())?;
            let status = Command::new("osascript")
                .args([
                    "-e",
                    "on run argv",
                    "-e",
                    "do shell script \"rm -rf \" & quoted form of item 1 of argv & \" && mv -f \" & quoted form of item 2 of argv & \" \" & quoted form of item 1 of argv with administrator privileges",
                    "-e",
                    "end run",
                ])
                .arg(&bundle)
                .arg(&fresh)
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "the update was not authorised",
                ))
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn install(bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::path::PathBuf;
    use std::{env, fs};

    let appimage = PathBuf::from(env::var_os("APPIMAGE").ok_or_else(|| {
        io::Error::new(io::ErrorKind::Unsupported, "not running from an AppImage")
    })?);
    let parent = appimage
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the AppImage has no parent"))?;
    let mut staged = tempfile::Builder::new()
        .prefix(".margin-update")
        .tempfile_in(parent)?;
    staged.write_all(bytes)?;
    staged.as_file().sync_all()?;
    staged
        .as_file()
        .set_permissions(fs::metadata(&appimage)?.permissions())?;
    staged.persist(&appimage).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn install(bytes: &[u8]) -> io::Result<()> {
    use std::env;
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::{self, Command};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const SCRIPT: &str = "Wait-Process -Id $env:MARGIN_PID -ErrorAction SilentlyContinue; Start-Process -FilePath $env:MARGIN_INSTALLER -ArgumentList '/P','/R'";

    let mut installer = tempfile::Builder::new()
        .prefix("margin-update-")
        .suffix(".exe")
        .tempfile()?;
    installer.write_all(bytes)?;
    let (file, path) = installer.keep().map_err(|error| error.error)?;
    drop(file);
    let powershell = env::var_os("SYSTEMROOT").map_or_else(
        || PathBuf::from("powershell.exe"),
        |root| PathBuf::from(root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"),
    );
    Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            SCRIPT,
        ])
        .env("MARGIN_PID", process::id().to_string())
        .env("MARGIN_INSTALLER", path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    Ok(())
}
