use std::collections::BTreeMap;

use crate::inline::LinkTarget;
use crate::text::{char_len, leading_spaces};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkDefinitions(BTreeMap<String, LinkTarget>);

impl LinkDefinitions {
    #[must_use]
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub(crate) fn insert(&mut self, label: &str, target: LinkTarget) {
        self.0.entry(normalize_label(label)).or_insert(target);
    }

    #[must_use]
    pub fn get(&self, label: &str) -> Option<&LinkTarget> {
        self.0.get(&normalize_label(label))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn labels(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = (&str, &LinkTarget)> {
        self.0
            .iter()
            .map(|(label, target)| (label.as_str(), target))
    }

    pub(crate) fn get_normalized(&self, label: &str) -> Option<&LinkTarget> {
        self.0.get(label)
    }
}

#[must_use]
pub(crate) fn normalize_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut pending_space = false;
    for ch in label.trim().chars() {
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.extend(ch.to_lowercase());
    }
    out
}

fn bracket_end(text: &str, from: usize) -> Option<usize> {
    let mut at = from;
    while at < text.len() {
        match text.as_bytes().get(at) {
            Some(b'\\') => at += 2,
            Some(b']') => return Some(at),
            Some(b'[') => return None,
            Some(_) => at += char_len(text, at),
            None => break,
        }
    }
    None
}

fn destination_end(text: &str, from: usize) -> usize {
    let mut at = from;
    let mut depth = 0;
    while at < text.len() {
        match text.as_bytes().get(at) {
            Some(b'\\') => at += 2,
            Some(b'(') => {
                depth += 1;
                at += 1;
            }
            Some(b')') => {
                if depth == 0 {
                    return at;
                }
                depth -= 1;
                at += 1;
            }
            Some(b' ' | b'\t' | b'\n' | b'\r') => return at,
            Some(_) => at += char_len(text, at),
            None => break,
        }
    }
    at
}

fn unquote_destination(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some(next) => out.push(next),
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn title_end(text: &str, from: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let open = *bytes.get(from)?;
    let close = if open == b'(' { b')' } else { open };
    let mut at = from + 1;
    while at < text.len() {
        match bytes.get(at) {
            Some(b'\\') => at += 2,
            Some(byte) if *byte == close => {
                let value = text.get(from + 1..at)?;
                return Some((unquote_destination(value), at + 1));
            }
            Some(_) => at += char_len(text, at),
            None => break,
        }
    }
    None
}

#[must_use]
pub(crate) fn parse_definition(text: &str) -> Option<(&str, LinkTarget)> {
    if leading_spaces(text) > 3 {
        return None;
    }
    let start = leading_spaces(text);
    let body = text.get(start..)?;
    if !body.starts_with('[') {
        return None;
    }
    let label_end = bracket_end(body, 1)?;
    let label = body.get(1..label_end)?;
    if label.is_empty() {
        return None;
    }
    let rest = body.get(label_end + 1..)?;
    if !rest.starts_with(':') {
        return None;
    }
    let after_colon = rest.get(1..)?;
    let trimmed = after_colon.trim_start_matches([' ', '\t']);
    if trimmed.is_empty() {
        return None;
    }
    let (destination, used) = if trimmed.starts_with('<') {
        let close = trimmed.find('>')?;
        let value = trimmed.get(1..close)?;
        if value.contains(['\n', '<']) {
            return None;
        }
        (value.to_owned(), close + 1)
    } else {
        let end = destination_end(trimmed, 0);
        let value = trimmed.get(..end)?;
        if value.is_empty() {
            return None;
        }
        (unquote_destination(value), end)
    };
    let tail = trimmed.get(used..)?.trim_start_matches([' ', '\t']);
    let title = if tail.is_empty() {
        None
    } else if matches!(tail.as_bytes().first(), Some(b'"' | b'\'' | b'(')) {
        let (value, _) = title_end(tail, 0)?;
        Some(value)
    } else {
        return None;
    };
    Some((label, LinkTarget::new(destination, title)))
}
