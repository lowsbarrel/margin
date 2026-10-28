use std::fmt;
use std::fmt::Write;

use crate::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub start: usize,
    pub end: usize,
}

impl Line {
    #[must_use]
    pub(crate) fn text<'a>(&self, source: &'a str) -> &'a str {
        self.span().slice(source)
    }

    #[must_use]
    pub(crate) const fn span(&self) -> Span {
        Span::new(self.start, self.end)
    }
}

#[must_use]
pub(crate) fn lines_of(source: &str) -> Vec<Line> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    for (at, byte) in bytes.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }
        let end = if at > start && bytes.get(at - 1) == Some(&b'\r') {
            at - 1
        } else {
            at
        };
        lines.push(Line { start, end });
        start = at + 1;
    }
    lines.push(Line {
        start,
        end: source.len(),
    });
    lines
}

#[must_use]
pub(crate) fn line_at(lines: &[Line], offset: usize) -> usize {
    match lines.binary_search_by(|line| line.start.cmp(&offset)) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    }
}

pub(crate) fn push_fmt(out: &mut String, args: fmt::Arguments<'_>) {
    Write::write_fmt(out, args).expect("writing into a String never fails");
}

#[must_use]
pub(crate) fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

#[must_use]
pub(crate) fn indent_of(text: &str) -> usize {
    let mut width = 0;
    for ch in text.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width += 4 - width % 4,
            _ => break,
        }
    }
    width
}

#[must_use]
pub(crate) fn leading_spaces(text: &str) -> usize {
    text.len() - text.trim_start_matches(' ').len()
}

#[must_use]
pub(crate) fn char_len(text: &str, at: usize) -> usize {
    text.get(at..)
        .and_then(|rest| rest.chars().next())
        .map_or(1, char::len_utf8)
}

#[must_use]
pub(crate) fn byte_at(text: &str, at: usize) -> Option<u8> {
    text.as_bytes().get(at).copied()
}

#[must_use]
pub(crate) fn char_before(text: &str, at: usize) -> Option<char> {
    text.get(..at).and_then(|head| head.chars().next_back())
}

#[must_use]
pub(crate) fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..).and_then(|rest| rest.chars().next())
}

#[must_use]
pub(crate) fn trim_end_of(source: &str, mut end: usize, start: usize) -> usize {
    while end > start && byte_at(source, end - 1) == Some(b' ') {
        end -= 1;
    }
    end
}

#[must_use]
pub(crate) fn break_len(text: &str, at: usize) -> usize {
    match byte_at(text, at) {
        Some(b'\r') if byte_at(text, at + 1) == Some(b'\n') => 2,
        _ => 1,
    }
}

#[must_use]
pub(crate) fn strip_columns(text: &str, columns: usize) -> usize {
    let mut width = 0;
    let mut at = 0;
    while at < text.len() && width < columns {
        match byte_at(text, at) {
            Some(b' ') => {
                width += 1;
                at += 1;
            }
            Some(b'\t') => {
                width += 4 - width % 4;
                at += 1;
            }
            _ => break,
        }
    }
    at
}
