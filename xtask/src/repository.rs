use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

#[derive(Debug)]
pub(crate) struct Repository {
    root: PathBuf,
    files: Vec<String>,
}

impl Repository {
    pub(crate) fn open() -> io::Result<Self> {
        let root = PathBuf::from(git(&["rev-parse", "--show-toplevel"])?.trim());
        let listing = git(&[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])?;
        let mut files: Vec<String> = listing
            .split('\0')
            .filter(|file| !file.is_empty() && root.join(file).is_file())
            .map(str::to_owned)
            .collect();
        files.sort();
        files.dedup();
        Ok(Self { root, files })
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(String::as_str)
    }

    pub(crate) fn read(&self, file: &str) -> Option<String> {
        fs::read_to_string(self.root.join(file)).ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layer {
    Shared,
    Features,
    Adapters,
    Ui,
    Apps,
    Testing,
    Tooling,
}

impl Layer {
    pub(crate) fn of_manifest(manifest: &str) -> Option<Self> {
        if manifest == "xtask/Cargo.toml" {
            return Some(Self::Tooling);
        }
        let mut parts = manifest.split('/');
        let (Some("crates"), Some(layer), Some(_name), Some("Cargo.toml"), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return None;
        };
        match layer {
            "shared" => Some(Self::Shared),
            "features" => Some(Self::Features),
            "adapters" => Some(Self::Adapters),
            "ui" => Some(Self::Ui),
            "apps" => Some(Self::Apps),
            "testing" => Some(Self::Testing),
            _ => None,
        }
    }

    pub(crate) fn is_core(self) -> bool {
        matches!(self, Self::Shared | Self::Features)
    }
}

fn git(arguments: &[&str]) -> io::Result<String> {
    let output = Command::new("git").args(arguments).output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    String::from_utf8(output.stdout).map_err(io::Error::other)
}

pub(crate) fn has_extension(file: &str, extension: &str) -> bool {
    Path::new(file)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}
