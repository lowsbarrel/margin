use std::collections::HashSet;
use std::time::Duration;

use sha2::{Digest, Sha256};

pub const SWEEP_GRACE: Duration = Duration::from_hours(7 * 24);

const HASH_CHARS: usize = 8;
const MAX_STEM: usize = 64;
const MAX_EXT: usize = 12;
pub(crate) const MAX_ATTEMPTS: u32 = 1000;

#[must_use]
pub(crate) fn attachment_name(original: &str, bytes: &[u8]) -> String {
    let name = base_name(original);
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, sanitize_ext(extension)),
        _ => (name.as_str(), String::new()),
    };
    let mut stored = format!("{}-{}", sanitize_stem(stem), hash_prefix(bytes));
    if !extension.is_empty() {
        stored.push('.');
        stored.push_str(&extension);
    }
    stored
}

#[must_use]
pub(crate) fn suffixed(name: &str, attempt: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) => format!("{stem}-{attempt}.{extension}"),
        None => format!("{name}-{attempt}"),
    }
}

#[must_use]
pub(crate) fn is_stored_name(name: &str) -> bool {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    has_hash_tail(stem)
        || stem.rsplit_once('-').is_some_and(|(head, number)| {
            !number.is_empty()
                && number.bytes().all(|byte| byte.is_ascii_digit())
                && has_hash_tail(head)
        })
}

fn has_hash_tail(stem: &str) -> bool {
    stem.rsplit_once('-').is_some_and(|(head, tail)| {
        !head.is_empty()
            && tail.len() == HASH_CHARS
            && tail
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn sanitize_stem(stem: &str) -> String {
    let mut mapped = String::with_capacity(stem.len());
    for character in stem.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '.') {
            mapped.push(character);
        } else if !mapped.ends_with('_') {
            mapped.push('_');
        }
    }
    let capped = &mapped[..mapped.len().min(MAX_STEM)];
    match capped.trim_matches(|character| character == '.' || character == '_') {
        "" => String::from("file"),
        trimmed => trimmed.to_owned(),
    }
}

fn sanitize_ext(extension: &str) -> String {
    extension
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(MAX_EXT)
        .collect::<String>()
        .to_ascii_lowercase()
}

fn base_name(original: &str) -> String {
    let normalised = original.replace('\\', "/");
    match normalised.rsplit('/').next() {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => original.to_owned(),
    }
}

fn hash_prefix(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let encoded = hex::encode(digest);
    encoded[..HASH_CHARS].to_owned()
}

pub(crate) fn referenced_names(markdown: &str, referenced: &mut HashSet<String>) {
    let mut position = 0;
    while let Some(found) = markdown[position..].find("![[") {
        let start = position + found + 3;
        match markdown[start..].find("]]") {
            Some(close) => {
                target(&markdown[start..start + close], referenced);
                position = start + close + 2;
            }
            None => break,
        }
    }

    let mut position = 0;
    while let Some(found) = markdown[position..].find("](") {
        let start = position + found + 2;
        match markdown[start..].find(')') {
            Some(close) => {
                target(&markdown[start..start + close], referenced);
                position = start + close + 1;
            }
            None => break,
        }
    }

    let mut position = 0;
    while let Some(found) = markdown[position..].find("src=") {
        let start = position + found + 4;
        let quote = markdown[start..].chars().next().unwrap_or(' ');
        let (body_start, end) = if quote == '"' || quote == '\'' {
            (
                start + quote.len_utf8(),
                markdown[start + 1..]
                    .find(quote)
                    .map(|offset| start + 1 + offset),
            )
        } else {
            (
                start,
                markdown[start..]
                    .find(|character: char| character.is_whitespace() || character == '>')
                    .map(|offset| start + offset),
            )
        };
        match end {
            Some(end) => {
                target(&markdown[body_start..end], referenced);
                position = end;
            }
            None => break,
        }
    }
}

fn target(raw: &str, referenced: &mut HashSet<String>) {
    let raw = raw.trim().trim_start_matches('<').trim_end_matches('>');
    let raw = raw.split_whitespace().next().unwrap_or("");
    let raw = raw.split('|').next().unwrap_or(raw);
    if raw.is_empty() {
        return;
    }
    let decoded = percent_decode_lossy(raw).replace('\\', "/");
    if let Some(name) = decoded.rsplit('/').next()
        && !name.is_empty()
    {
        referenced.insert(name.to_owned());
    }
}

fn percent_decode_lossy(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut decoded: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%'
            && let (Some(high), Some(low)) = (
                hex_value_at(bytes, index + 1),
                hex_value_at(bytes, index + 2),
            )
        {
            decoded.push(high * 16 + low);
            index += 3;
            continue;
        }
        decoded.push(byte);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_value_at(bytes: &[u8], index: usize) -> Option<u8> {
    bytes.get(index).copied().and_then(hex_value)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_dot_is_stripped_from_the_stem() {
        assert_eq!(sanitize_stem("../../etc/passwd"), "etc_passwd");
        assert_eq!(sanitize_stem("!!!"), "file");
        assert_eq!(sanitize_stem(".hidden"), "hidden");
    }

    #[test]
    fn percent_escapes_in_a_reference_decode_to_their_text() {
        let mut referenced = HashSet::new();
        target("attachments/space%20name.png", &mut referenced);
        assert_eq!(referenced, HashSet::from([String::from("space name.png")]));
    }

    #[test]
    fn stored_names_are_recognised_by_their_hash_tail() {
        assert!(is_stored_name("shot-0a1b2c3d.png"));
        assert!(is_stored_name("shot-0a1b2c3d-2.png"));
        assert!(is_stored_name("noext-12345678"));
        assert!(!is_stored_name("shot.png"));
        assert!(!is_stored_name("shot-0A1B2C3D.png"));
        assert!(!is_stored_name("shot-0a1b2c3.png"));
        assert!(!is_stored_name("-0a1b2c3d.png"));
    }
}
