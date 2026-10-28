use std::ffi::OsStr;
use std::io::Error;
use std::path::Path;
use std::process::{Command, Stdio};

use margin_kernel::StoreError;

pub fn reveal_in_file_manager(path: &Path) -> Result<(), StoreError> {
    if !path.exists() {
        return Err(StoreError::NotFound(path.display().to_string()));
    }
    #[cfg(target_os = "macos")]
    let result = reveal_on_macos(path);
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = reveal_on_linux(path);
    #[cfg(target_os = "windows")]
    let result = reveal_on_windows(path);
    result
}

pub fn open_with_default_app(target: &str) -> Result<(), StoreError> {
    #[cfg(target_os = "macos")]
    let result = run("open", &[OsStr::new(target)]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = run("xdg-open", &[OsStr::new(target)]);
    #[cfg(target_os = "windows")]
    let result = run(
        "cmd",
        &[
            OsStr::new("/C"),
            OsStr::new("start"),
            OsStr::new(""),
            OsStr::new(target),
        ],
    );
    result
}

fn command(program: &str, arguments: &[&OsStr]) -> Command {
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn run(program: &str, arguments: &[&OsStr]) -> Result<(), StoreError> {
    let status = command(program, arguments)
        .status()
        .map_err(|error| launch_failed(program, &error))?;
    if status.success() {
        Ok(())
    } else {
        Err(StoreError::Other(format!(
            "{program} exited with status {status}"
        )))
    }
}

fn launch_failed(program: &str, error: &Error) -> StoreError {
    StoreError::Other(format!("cannot launch {program}: {error}"))
}

#[cfg(target_os = "macos")]
fn reveal_on_macos(path: &Path) -> Result<(), StoreError> {
    if path.is_dir() {
        run("open", &[path.as_os_str()])
    } else {
        run("open", &[OsStr::new("-R"), path.as_os_str()])
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_on_linux(path: &Path) -> Result<(), StoreError> {
    if select_in_the_file_manager(path).is_ok() {
        return Ok(());
    }
    let directory = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    run("xdg-open", &[directory.as_os_str()])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn select_in_the_file_manager(path: &Path) -> Result<(), StoreError> {
    let selection = format!("array:string:{}", file_url(path));
    run(
        "dbus-send",
        &[
            OsStr::new("--session"),
            OsStr::new("--print-reply"),
            OsStr::new("--dest=org.freedesktop.FileManager1"),
            OsStr::new("/org/freedesktop/FileManager1"),
            OsStr::new("org.freedesktop.FileManager1.ShowItems"),
            OsStr::new(&selection),
            OsStr::new("string:"),
        ],
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'.' | b'_' | b'~' => {
                url.push(char::from(byte))
            }
            _ => {
                url.push('%');
                url.push(hex_digit(byte >> 4));
                url.push(hex_digit(byte & 0x0f));
            }
        }
    }
    url
}

#[cfg(all(unix, not(target_os = "macos")))]
fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        _ => char::from(b'A' + value - 10),
    }
}

#[cfg(target_os = "windows")]
fn reveal_on_windows(path: &Path) -> Result<(), StoreError> {
    use std::os::windows::process::CommandExt;
    let native = path.to_string_lossy().replace('/', "\\");
    let mut command = windows_explorer_command();
    if path.is_dir() {
        command.arg(&native);
    } else {
        command.raw_arg(format!("/select,{native}"));
    }
    let _status_ignored = command
        .status()
        .map_err(|error| launch_failed("explorer", &error))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_explorer_command() -> Command {
    let mut command = Command::new("explorer");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}
