use std::path::{Path, PathBuf};
use std::str;

use margin_kernel::RelativePath;

use crate::vault_index::VaultIndex;

const LOCALFILE_PREFIXES: [&str; 2] = ["http://localfile.localhost", "localfile://localhost"];
const IMAGE_EXTENSIONS: [&str; 11] = [
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "ico", "tiff", "tif",
];

pub(crate) fn decode_destination(path: &str) -> String {
    if !path.contains('%') {
        return path.to_owned();
    }
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        let escaped = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| str::from_utf8(pair).ok())
            .filter(|pair| byte == b'%' && pair.bytes().all(|digit| digit.is_ascii_hexdigit()))
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        if let Some(value) = escaped {
            decoded.push(value);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

pub(crate) fn normalized(path: &str) -> Option<RelativePath> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return None;
    }
    RelativePath::new(&parts.join("/")).ok()
}

pub(crate) fn note_dir(note: &RelativePath) -> RelativePath {
    note.parent().unwrap_or_default()
}

pub(crate) fn joined(folder: &RelativePath, path: &str) -> String {
    if folder.is_root() {
        path.to_owned()
    } else {
        format!("{folder}/{path}")
    }
}

pub(crate) fn inside_vault(root: &Path, path: &Path) -> Option<RelativePath> {
    let rest = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rest
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    RelativePath::new(&parts.join("/"))
        .ok()
        .filter(|path| !path.is_root())
}

pub(crate) fn has_scheme(raw: &str) -> bool {
    let mut characters = raw.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && raw
            .find(':')
            .and_then(|colon| raw.get(1..colon))
            .is_some_and(|rest| {
                rest.chars()
                    .all(|character| character.is_alphanumeric() || "_+.-".contains(character))
            })
}

pub(crate) fn is_web_image(raw: &str) -> bool {
    let lower = raw.trim().to_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:image/")
}

pub(crate) fn is_remote(raw: &str) -> bool {
    let lower = raw.trim().to_lowercase();
    ["http:", "https:", "data:", "blob:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

pub(crate) fn localfile_path(raw: &str) -> Option<PathBuf> {
    let tail = LOCALFILE_PREFIXES
        .iter()
        .find_map(|prefix| raw.strip_prefix(prefix))?;
    Some(PathBuf::from(os_path(&decode_destination(tail))))
}

fn os_path(path: &str) -> String {
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    let bytes = trimmed.as_bytes();
    let is_drive = bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
        && matches!(bytes.get(2), Some(b'/' | b'\\'));
    if is_drive {
        trimmed.replace('/', "\\")
    } else {
        path.to_owned()
    }
}

pub(crate) fn extension_of(name: &str) -> String {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[dot + 1..].to_lowercase(),
        _ => String::new(),
    }
}

pub(crate) fn is_image_path(name: &str) -> bool {
    let extension = name.rsplit('.').next().unwrap_or_default().to_lowercase();
    IMAGE_EXTENSIONS.contains(&extension.as_str())
}

pub(crate) fn is_note_path(path: &RelativePath) -> bool {
    path.has_extension("md") || path.has_extension("canvas")
}

pub(crate) fn find_variant(
    index: &VaultIndex,
    note: &RelativePath,
    variants: &[String],
) -> Option<RelativePath> {
    let first = variants.first()?;
    if first.contains('/') {
        return variants
            .iter()
            .filter_map(|variant| normalized(variant))
            .find(|path| index.exists(path));
    }
    let roots = [
        note_dir(note),
        RelativePath::root(),
        index.attachment_folder().clone(),
    ];
    for root in &roots {
        for variant in variants {
            if let Some(path) = normalized(&joined(root, variant))
                && index.exists(&path)
            {
                return Some(path);
            }
        }
    }
    variants
        .iter()
        .find_map(|variant| index.find_by_name(variant))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_escapes_decode_as_utf8() {
        assert_eq!(decode_destination("a%20b%C3%A8.md"), "a b\u{e8}.md");
        assert_eq!(decode_destination("100%"), "100%");
        assert_eq!(decode_destination("%zz"), "%zz");
    }

    #[test]
    fn paths_that_climb_out_of_the_vault_do_not_normalise() {
        assert_eq!(
            normalized("a/./b/../c.md").map(String::from),
            Some(String::from("a/c.md"))
        );
        assert_eq!(normalized("../outside.md"), None);
        assert_eq!(normalized("a/../../x"), None);
        assert_eq!(
            normalized("a\\b.png").map(String::from),
            Some(String::from("a/b.png"))
        );
    }

    #[test]
    fn schemes_and_drive_letters_are_told_apart() {
        assert!(has_scheme("https://x"));
        assert!(has_scheme("obsidian://open"));
        assert!(!has_scheme("notes/a.md"));
        assert!(!has_scheme("a b:c"));
        assert_eq!(
            localfile_path("localfile://localhost/C:/Users/a%20b.png"),
            Some(PathBuf::from("C:\\Users\\a b.png"))
        );
        assert_eq!(
            localfile_path("http://localfile.localhost/vault/x.png"),
            Some(PathBuf::from("/vault/x.png"))
        );
    }

    #[test]
    fn extensions_classify_images() {
        assert!(is_image_path("Shot.PNG"));
        assert!(!is_image_path("doc.pdf"));
        assert_eq!(extension_of(".hidden"), "");
        assert_eq!(extension_of("a.tar.GZ"), "gz");
    }
}
