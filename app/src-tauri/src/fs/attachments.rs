//! Attachment storage and orphan detection.
//!
//! An attachment is named `<sanitized stem>-<first 8 hex of sha256>.<ext>` and
//! written once: the hash makes the name a function of the content, so pasting
//! the same image twice reuses the file instead of accumulating copies, and an
//! existing file is never replaced.

use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

use super::{WalkAction, normalise_slashes, path_to_string, walk_dir};

/// Characters of the hex digest kept in the name. Eight hex digits is 32 bits —
/// a collision between two different files is possible in principle, which is
/// why [`store`] never assumes the name is free.
const HASH_CHARS: usize = 8;

/// Longest stem kept. A clipboard file name can be near the filesystem limit on
/// its own, and the hash and a further suffix still have to fit after it.
const MAX_STEM: usize = 64;

/// Longest extension kept. Extensions are short; this only stops a pathological
/// name from growing the file name without bound.
const MAX_EXT: usize = 12;

/// How many name collisions to resolve before giving up. Reaching this means
/// thousands of same-named, different-content files.
const MAX_ATTEMPTS: u32 = 1000;

/// Keep the characters that are portable in a file name, collapsing every run of
/// rejected ones into a single `_`, then drop the leading/trailing dots and
/// underscores: a name starting with `.` is hidden from every walker, the
/// watcher and sync.
fn sanitize_stem(stem: &str) -> String {
    let mut mapped = String::with_capacity(stem.len());
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
            mapped.push(c);
        } else if !mapped.ends_with('_') {
            mapped.push('_');
        }
    }
    // `mapped` is ASCII, so capping never splits a character.
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

/// The bare file name of `original`, with any directory part dropped.
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

/// `stem-hash.ext` — the name an attachment's content always gets.
fn attachment_name(original: &str, bytes: &[u8]) -> String {
    let name = base_name(original);
    let (stem, ext) = match name.rsplit_once('.') {
        // A leading dot belongs to the stem (dotfile), not to an extension.
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

/// The `n`th candidate for a taken name: `stem-hash-2.ext`.
fn suffixed(name: &str, n: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{n}.{ext}"),
        None => format!("{name}-{n}"),
    }
}

/// Store `bytes` under a content-addressed name, returning the file name.
///
/// An existing file with identical content is reused; anything else at the
/// chosen name is left untouched and the next candidate is tried. Creation uses
/// `create_new`, so the write itself refuses to replace a file that appeared
/// between the check and the write.
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

/// Read `src` and store it as an attachment; the vault-relative path comes back.
pub(crate) fn import_file(src: &Path, dir: &Path, rel_dir: &str) -> Result<String, String> {
    let bytes = fs::read(src).map_err(|e| format!("Failed to read the dropped file: {e}"))?;
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(format!("{rel_dir}/{}", store(dir, &name, &bytes)?))
}

/// Every file name a note can point at.
///
/// Only names are compared — never resolved paths — because a name match can
/// only ever *keep* a file: two attachments that happen to share a name are
/// either both referenced or both reported, never one of each.
fn referenced_names(md: &str, out: &mut HashSet<String>) {
    let mut pos = 0;
    while let Some(found) = md[pos..].find("![[") {
        let start = pos + found + 3;
        match md[start..].find("]]") {
            Some(close) => {
                target(&md[start..start + close], out);
                pos = start + close + 2;
            }
            None => break,
        }
    }

    // Markdown images and links alike: `![alt](dest)` and `[text](dest)` differ
    // only in the `!`, which this scan is indifferent to.
    let mut pos = 0;
    while let Some(found) = md[pos..].find("](") {
        let start = pos + found + 2;
        match md[start..].find(')') {
            Some(close) => {
                target(&md[start..start + close], out);
                pos = start + close + 1;
            }
            None => break,
        }
    }

    // The one HTML attribute a note can carry that names a file.
    let mut pos = 0;
    while let Some(found) = md[pos..].find("src=") {
        let start = pos + found + 4;
        let quote = md[start..].chars().next().unwrap_or(' ');
        let (body_start, end) = if quote == '"' || quote == '\'' {
            (
                start + quote.len_utf8(),
                md[start + 1..].find(quote).map(|i| start + 1 + i),
            )
        } else {
            (
                start,
                md[start..]
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .map(|i| start + i),
            )
        };
        match end {
            Some(end) => {
                target(&md[body_start..end], out);
                pos = end;
            }
            None => break,
        }
    }
}

/// Add the file name a single destination or embed target points at.
fn target(raw: &str, out: &mut HashSet<String>) {
    let raw = raw.trim().trim_start_matches('<').trim_end_matches('>');
    // A destination may be followed by a title; an embed may carry a size alias.
    let raw = raw.split_whitespace().next().unwrap_or("");
    let raw = raw.split('|').next().unwrap_or(raw);
    if raw.is_empty() {
        return;
    }
    let decoded = percent_encoding::percent_decode_str(raw).decode_utf8_lossy();
    let decoded = normalise_slashes(&decoded);
    if let Some(name) = decoded.rsplit('/').next()
        && !name.is_empty()
    {
        out.insert(name.to_string());
    }
}

/// Files inside `folder` that no `.md` in `root` refers to, vault-relative and
/// sorted. Returns nothing when the folder does not exist.
pub(crate) fn unused_files(root: &str, folder: &str) -> Result<Vec<String>, String> {
    let dir = Path::new(root).join(folder);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut referenced: HashSet<String> = HashSet::new();
    let mut notes: Vec<std::path::PathBuf> = Vec::new();
    walk_dir(Path::new(root), &mut |item| {
        if item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        if item.is_dir {
            return WalkAction::Recurse;
        }
        if item
            .path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        {
            notes.push(item.path.clone());
        }
        WalkAction::Skip
    });

    for note in notes {
        if let Ok(md) = fs::read_to_string(&note) {
            referenced_names(&md, &mut referenced);
        }
    }

    let mut unused: Vec<String> = Vec::new();
    walk_dir(&dir, &mut |item| {
        if item.name.starts_with('.') {
            return WalkAction::Skip;
        }
        if item.is_dir {
            return WalkAction::Recurse;
        }
        if referenced.contains(&item.name) {
            return WalkAction::Skip;
        }
        let rel = item
            .path
            .strip_prefix(&dir)
            .map(|rel| path_to_string(rel.to_path_buf()))
            .unwrap_or_else(|_| item.name.clone());
        unused.push(format!("{folder}/{rel}"));
        WalkAction::Skip
    });
    unused.sort();
    Ok(unused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "margin-attachments-test-{}-{tag}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_name_is_the_sanitized_stem_plus_the_content_hash() {
        let hash = hash_prefix(b"abc");
        assert_eq!(
            attachment_name("My? Photo.png", b"abc"),
            format!("My_Photo-{hash}.png")
        );
        assert_eq!(
            attachment_name("shot.PNG", b"abc"),
            format!("shot-{hash}.png")
        );
        assert_eq!(
            attachment_name("archive", b"abc"),
            format!("archive-{hash}")
        );
        assert_eq!(attachment_name(".hidden", b"abc"), format!("hidden-{hash}"));
        assert_eq!(attachment_name("a/.b/.png", b"abc"), format!("png-{hash}"));
        assert_eq!(
            attachment_name(&format!("{}.png", "a".repeat(200)), b"abc"),
            format!("{}-{hash}.png", "a".repeat(MAX_STEM))
        );

        assert_eq!(sanitize_stem("../../etc/passwd"), "etc_passwd");
        assert_eq!(sanitize_stem("!!!"), "file");
        assert_eq!(sanitize_stem(".hidden"), "hidden");
    }

    #[test]
    fn the_same_content_reuses_its_file() {
        let dir = temp_dir("dedupe");
        let first = store(&dir, "shot.png", b"same bytes").unwrap();
        let second = store(&dir, "shot.png", b"same bytes").unwrap();

        assert_eq!(first, second, "identical content must resolve to one file");
        assert_eq!(files_in(&dir), vec![first]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_taken_name_is_never_overwritten() {
        let dir = temp_dir("collision");
        // The name this content would take is already held by a different file.
        let taken = format!("shot-{}.png", hash_prefix(b"second"));
        fs::write(dir.join(&taken), "the original").unwrap();

        let stored = store(&dir, "shot.png", b"second").unwrap();

        assert_ne!(stored, taken, "a different file must not take the name");
        assert_eq!(stored, format!("shot-{}-2.png", hash_prefix(b"second")));
        assert_eq!(
            fs::read_to_string(dir.join(&taken)).unwrap(),
            "the original",
            "the file that was there must be untouched"
        );
        assert_eq!(files_in(&dir).len(), 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn importing_reads_the_source_and_keeps_its_name() {
        let dir = temp_dir("import");
        let src = dir.join("holiday.JPEG");
        fs::write(&src, b"photo").unwrap();
        let dest = dir.join("attachments");

        let rel = import_file(&src, &dest, "attachments").unwrap();

        assert_eq!(
            rel,
            format!("attachments/holiday-{}.jpeg", hash_prefix(b"photo"))
        );
        assert_eq!(
            fs::read(dir.join("attachments").join(&rel["attachments/".len()..])).unwrap(),
            b"photo"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_unreferenced_attachments_are_reported() {
        let root = temp_dir("unused");
        let notes = root.join("notes");
        let att = root.join("attachments");
        fs::create_dir_all(&notes).unwrap();
        fs::create_dir_all(&att).unwrap();
        fs::write(root.join("index.md"), "![a](attachments/used.png)").unwrap();
        fs::write(notes.join("b.md"), "![[embedded.pdf]]").unwrap();
        fs::write(
            notes.join("c.md"),
            "<img src=\"attachments/space%20name.png\">\n[spaced](<attachments/angle.png>)",
        )
        .unwrap();
        for name in [
            "used.png",
            "embedded.pdf",
            "space name.png",
            "angle.png",
            "orphan.png",
        ] {
            fs::write(att.join(name), b"x").unwrap();
        }

        let unused = unused_files(&root.to_string_lossy(), "attachments").unwrap();

        assert_eq!(unused, vec!["attachments/orphan.png".to_string()]);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_hidden_note_does_not_keep_a_file() {
        let root = temp_dir("unused-hidden");
        let att = root.join("attachments");
        fs::create_dir_all(root.join(".margin")).unwrap();
        fs::create_dir_all(&att).unwrap();
        fs::write(
            root.join(".margin/backup.md"),
            "![a](attachments/orphan.png)",
        )
        .unwrap();
        fs::write(att.join("orphan.png"), b"x").unwrap();

        let unused = unused_files(&root.to_string_lossy(), "attachments").unwrap();

        assert_eq!(unused, vec!["attachments/orphan.png".to_string()]);

        fs::remove_dir_all(&root).ok();
    }
}
