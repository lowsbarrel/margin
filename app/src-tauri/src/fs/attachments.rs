use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::Path;

pub(crate) mod commands;
mod scan;
#[cfg(test)]
mod tests;

use super::normalise_slashes;

const HASH_CHARS: usize = 8;
const MAX_STEM: usize = 64;
const MAX_EXT: usize = 12;
const MAX_ATTEMPTS: u32 = 1000;

fn sanitize_stem(stem: &str) -> String {
    let mut mapped = String::with_capacity(stem.len());
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
            mapped.push(c);
        } else if !mapped.ends_with('_') {
            mapped.push('_');
        }
    }
    let capped = &mapped[..mapped.len().min(MAX_STEM)];
    let trimmed = capped.trim_matches(|c| c == '.' || c == '_');
    match trimmed {
        "" => "file".to_string(),
        s => s.to_string(),
    }
}

fn sanitize_ext(ext: &str) -> String {
    ext.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(MAX_EXT)
        .collect::<String>()
        .to_ascii_lowercase()
}

fn base_name(original: &str) -> String {
    let normalised = normalise_slashes(original);
    match normalised.rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => original.to_string(),
    }
}

fn hash_prefix(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)[..HASH_CHARS].to_string()
}

// An attachment is named `<sanitized stem>-<8 hex sha256>[-n].<ext>`; the sweep recognises its own files by that tail.
fn attachment_name(original: &str, bytes: &[u8]) -> String {
    let name = base_name(original);
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, sanitize_ext(ext)),
        _ => (name.as_str(), String::new()),
    };
    let mut out = format!("{}-{}", sanitize_stem(stem), hash_prefix(bytes));
    if !ext.is_empty() {
        out.push('.');
        out.push_str(&ext);
    }
    out
}

fn suffixed(name: &str, n: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{n}.{ext}"),
        None => format!("{name}-{n}"),
    }
}

pub(crate) fn store(dir: &Path, original_name: &str, bytes: &[u8]) -> Result<String, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create the attachments folder: {e}"))?;

    let name = attachment_name(original_name, bytes);
    for attempt in 0..MAX_ATTEMPTS {
        let candidate = match attempt {
            0 => name.clone(),
            n => suffixed(&name, n + 1),
        };
        let path = dir.join(&candidate);
        match write_new(&path, bytes) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if fs::read(&path).is_ok_and(|existing| existing == bytes) {
                    return Ok(candidate);
                }
            }
            Err(e) => return Err(format!("Failed to write the attachment: {e}")),
        }
    }
    Err("Could not find a free name for the attachment".to_string())
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)
}

pub(crate) fn import_file(src: &Path, dir: &Path, rel_dir: &str) -> Result<String, String> {
    let bytes = fs::read(src).map_err(|e| format!("Failed to read the dropped file: {e}"))?;
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(format!("{rel_dir}/{}", store(dir, &name, &bytes)?))
}
