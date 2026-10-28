use crate::autolink;
use crate::inline::{EmbedTarget, Inline, InlineKind, WikiLink};
use crate::inline_parse::Entry;
use crate::span::Span;
use crate::text::{break_len, byte_at, char_at, char_before, char_len};

pub(crate) fn line_end(text: &str, from: usize) -> usize {
    text.get(from..)
        .and_then(|rest| rest.find('\n'))
        .map_or(text.len(), |offset| from + offset)
}

pub(crate) fn find_byte(text: &str, from: usize, to: usize, byte: u8) -> Option<usize> {
    let slice = text.get(from..to)?;
    slice
        .as_bytes()
        .iter()
        .position(|value| *value == byte)
        .map(|offset| from + offset)
}

pub(crate) fn skip_space(text: &str, from: usize) -> usize {
    let mut at = from;
    while matches!(byte_at(text, at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        at += 1;
    }
    at
}

pub(crate) fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some(next) if next.is_ascii_punctuation() => out.push(next),
            Some(next) => {
                out.push('\\');
                out.push(next);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn node(kind: InlineKind, span: Span, content: Span, markers: Vec<Span>) -> Entry {
    Entry::Node(Inline::new(kind, span, content, markers))
}

pub(crate) fn escape(text: &str, base: usize, at: usize) -> Option<(Entry, usize)> {
    let next = byte_at(text, at + 1)?;
    if next == b'\n' {
        return Some((
            node(
                InlineKind::HardBreak,
                Span::new(base + at, base + at + 1),
                Span::empty(base + at),
                vec![Span::new(base + at, base + at + 1)],
            ),
            at + 2,
        ));
    }
    if next == b'\r' {
        let width = break_len(text, at + 1);
        return Some((
            node(
                InlineKind::HardBreak,
                Span::new(base + at, base + at + 1),
                Span::empty(base + at),
                vec![Span::new(base + at, base + at + 1)],
            ),
            at + 1 + width,
        ));
    }
    if !next.is_ascii_punctuation() {
        return None;
    }
    Some((
        node(
            InlineKind::Escape,
            Span::new(base + at, base + at + 2),
            Span::new(base + at + 1, base + at + 2),
            vec![Span::new(base + at, base + at + 1)],
        ),
        at + 2,
    ))
}

pub(crate) fn code(text: &str, base: usize, at: usize) -> Option<(Entry, usize)> {
    let mut count = 0;
    while byte_at(text, at + count) == Some(b'`') {
        count += 1;
    }
    let open_end = at + count;
    let mut scan = open_end;
    while scan < text.len() {
        if byte_at(text, scan) != Some(b'`') {
            scan += char_len(text, scan);
            continue;
        }
        let mut run = 0;
        while byte_at(text, scan + run) == Some(b'`') {
            run += 1;
        }
        if run == count {
            let inner = text.get(open_end..scan)?;
            let mut from = open_end;
            let mut to = scan;
            if inner.starts_with(' ') && inner.ends_with(' ') && !inner.trim().is_empty() {
                from += 1;
                to -= 1;
            }
            return Some((
                node(
                    InlineKind::Code,
                    Span::new(base + at, base + scan + count),
                    Span::new(base + from, base + to),
                    vec![
                        Span::new(base + at, base + open_end),
                        Span::new(base + scan, base + scan + count),
                    ],
                ),
                scan + count,
            ));
        }
        scan += run;
    }
    None
}

fn html_tag(inner: &str) -> Option<(String, bool)> {
    if inner.starts_with("!--") {
        return Some(("!--".to_owned(), false));
    }
    if inner.starts_with('?') {
        return Some(("?".to_owned(), false));
    }
    if inner.starts_with('!') {
        return Some(("!".to_owned(), false));
    }
    let (closing, body) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let mut name = String::new();
    for ch in body.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            name.push(ch.to_ascii_lowercase());
        } else {
            break;
        }
    }
    if name.is_empty() || !name.starts_with(|first: char| first.is_ascii_alphabetic()) {
        return None;
    }
    Some((name, closing))
}

pub(crate) fn angle(text: &str, base: usize, at: usize) -> Option<(Entry, usize)> {
    let stop = text.len();
    let close = find_byte(text, at + 1, stop, b'>')?;
    let inner = text.get(at + 1..close)?;
    if inner.is_empty() {
        return None;
    }
    if let Some((destination, email)) = autolink::autolink_target(inner) {
        return Some((
            node(
                InlineKind::Autolink { destination, email },
                Span::new(base + at, base + close + 1),
                Span::new(base + at + 1, base + close),
                vec![
                    Span::new(base + at, base + at + 1),
                    Span::new(base + close, base + close + 1),
                ],
            ),
            close + 1,
        ));
    }
    let limit = if inner.starts_with("!--") {
        text.len()
    } else {
        line_end(text, at + 1)
    };
    if close >= limit {
        return None;
    }
    let (name, closing) = html_tag(inner)?;
    let span = Span::new(base + at, base + close + 1);
    Some((
        node(
            InlineKind::Html { name, closing },
            span,
            Span::empty(span.start),
            vec![span],
        ),
        close + 1,
    ))
}

pub(crate) fn math(text: &str, base: usize, at: usize) -> Option<(Entry, usize)> {
    if byte_at(text, at + 1) == Some(b'$') || (at > 0 && byte_at(text, at - 1) == Some(b'$')) {
        return None;
    }
    let first = char_at(text, at + 1)?;
    if first == ' ' {
        return None;
    }
    let stop = line_end(text, at + 1);
    let end = find_byte(text, at + 1, stop, b'$')?;
    let last = char_before(text, end)?;
    if last == ' ' || last == '\\' {
        return None;
    }
    if char_at(text, end + 1).is_some_and(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((
        node(
            InlineKind::Math,
            Span::new(base + at, base + end + 1),
            Span::new(base + at + 1, base + end),
            vec![
                Span::new(base + at, base + at + 1),
                Span::new(base + end, base + end + 1),
            ],
        ),
        end + 1,
    ))
}

pub(crate) fn comment(text: &str, base: usize, at: usize) -> Option<(Entry, usize)> {
    if byte_at(text, at + 1) != Some(b'%') {
        return None;
    }
    let rest = text.get(at + 2..)?;
    let offset = rest.find("%%")?;
    let end = at + 2 + offset;
    Some((
        node(
            InlineKind::Comment,
            Span::new(base + at, base + end + 2),
            Span::new(base + at + 2, base + end),
            vec![
                Span::new(base + at, base + at + 2),
                Span::new(base + end, base + end + 2),
            ],
        ),
        end + 2,
    ))
}

pub(crate) fn tag(text: &str, base: usize, at: usize) -> Option<(Entry, usize)> {
    if at > 0 {
        let before = char_before(text, at)?;
        if !matches!(before, ' ' | '\t' | '\n' | '\r' | '(' | '>' | '[' | ']') {
            return None;
        }
    }
    let stop = line_end(text, at + 1);
    let mut end = at + 1;
    while end < stop {
        let Some(ch) = char_at(text, end) else {
            break;
        };
        if ch.is_alphanumeric() || matches!(ch, '_' | '-' | '/') {
            end += ch.len_utf8();
        } else {
            break;
        }
    }
    while end > at + 1 && byte_at(text, end - 1) == Some(b'/') {
        end -= 1;
    }
    if end == at + 1 {
        return None;
    }
    let name = text.get(at + 1..end)?;
    if !name.chars().any(|ch| ch.is_alphabetic() || ch == '_') {
        return None;
    }
    Some((
        node(
            InlineKind::Tag {
                name: name.to_owned(),
            },
            Span::new(base + at, base + end),
            Span::new(base + at + 1, base + end),
            vec![Span::new(base + at, base + at + 1)],
        ),
        end,
    ))
}

pub(crate) fn footnote_ref(text: &str, base: usize, at: usize) -> Option<(Entry, usize)> {
    let stop = line_end(text, at + 2);
    let close = find_byte(text, at + 2, stop, b']')?;
    let id = text.get(at + 2..close)?;
    if id.is_empty() || id.chars().any(char::is_whitespace) {
        return None;
    }
    Some((
        node(
            InlineKind::FootnoteReference {
                id: id.to_lowercase(),
            },
            Span::new(base + at, base + close + 1),
            Span::new(base + at + 2, base + close),
            vec![
                Span::new(base + at, base + at + 2),
                Span::new(base + close, base + close + 1),
            ],
        ),
        close + 1,
    ))
}

pub(crate) fn wiki_link(text: &str, base: usize, at: usize, embed: bool) -> Option<(Entry, usize)> {
    let open_len = if embed { 3 } else { 2 };
    let inner_start = at + open_len;
    let stop = line_end(text, inner_start);
    let close = find_byte(text, inner_start, stop, b']')?;
    if byte_at(text, close + 1) != Some(b']') {
        return None;
    }
    let inner = text.get(inner_start..close)?;
    if inner.is_empty() || inner.contains('[') {
        return None;
    }
    let end = close + 2;
    let mut markers = vec![
        Span::new(base + at, base + inner_start),
        Span::new(base + close, base + end),
    ];
    let (target_part, alias_part, pipe_at) = match inner.find('|') {
        Some(offset) => (
            inner.get(..offset)?,
            Some(inner.get(offset + 1..)?),
            Some(offset),
        ),
        None => (inner, None, None),
    };
    if let Some(offset) = pipe_at {
        markers.push(Span::new(
            base + inner_start + offset,
            base + inner_start + offset + 1,
        ));
    }
    let hash_at = target_part.find('#');
    if let Some(offset) = hash_at {
        markers.push(Span::new(
            base + inner_start + offset,
            base + inner_start + offset + 1,
        ));
    }
    let path = match hash_at {
        Some(offset) => target_part.get(..offset)?,
        None => target_part,
    };
    let sub = hash_at.and_then(|offset| target_part.get(offset + 1..));
    let target = path.trim().to_owned();
    if target.is_empty() {
        return None;
    }
    let (heading, block) = match sub {
        Some(value) if value.starts_with('^') => {
            (None, Some(value.get(1..).unwrap_or_default().to_owned()))
        }
        Some(value) if !value.is_empty() => (Some(value.to_owned()), None),
        _ => (None, None),
    };
    let alias = alias_part
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let content = match pipe_at {
        Some(offset) => Span::new(base + inner_start + offset + 1, base + close),
        None => Span::new(base + inner_start, base + inner_start + target_part.len()),
    };
    let kind = if embed {
        let size = alias.as_deref().and_then(|value| value.parse::<u32>().ok());
        InlineKind::Embed(EmbedTarget { target, size })
    } else {
        InlineKind::WikiLink(Box::new(WikiLink {
            target,
            heading,
            block,
            alias,
        }))
    };
    Some((
        node(kind, Span::new(base + at, base + end), content, markers),
        end,
    ))
}
