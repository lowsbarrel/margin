use crate::inline::{Inline, InlineKind};
use crate::link_ref::LinkDefinitions;
use crate::span::Span;
use crate::text::{break_len, byte_at, char_at, char_before, char_len, trim_end_of};
use crate::{autolink, inline_atom, inline_link};

pub(crate) struct Ctx<'a> {
    pub source: &'a str,
    pub definitions: &'a LinkDefinitions,
}

pub(crate) enum Entry {
    Node(Inline),
    Delim(Delim),
}

#[derive(Clone, Copy)]
pub(crate) struct Delim {
    pub ch: u8,
    pub count: usize,
    pub can_open: bool,
    pub can_close: bool,
    pub span: Span,
}

impl<'a> Ctx<'a> {
    pub(crate) const fn new(source: &'a str, definitions: &'a LinkDefinitions) -> Self {
        Self {
            source,
            definitions,
        }
    }

    pub(crate) fn parse(&self, span: Span) -> Vec<Inline> {
        self.sequence(span, true)
    }

    pub(crate) fn sequence(&self, span: Span, allow_links: bool) -> Vec<Inline> {
        let text = span.slice(self.source);
        let base = span.start;
        let mut entries: Vec<Entry> = Vec::new();
        let mut at = 0;
        let mut plain = 0;
        let mut line_stop = inline_atom::line_end(text, 0);
        while at < text.len() {
            if at > line_stop {
                line_stop = inline_atom::line_end(text, at);
            }
            let Some(byte) = byte_at(text, at) else {
                break;
            };
            if byte == b'\r' || byte == b'\n' {
                let width = break_len(text, at);
                let end = trim_end_of(text, at, plain);
                flush(text, base, plain, end, &mut entries);
                let hard = at - end >= 2;
                let (kind, marker) = if hard {
                    (InlineKind::HardBreak, Span::new(base + end, base + at))
                } else {
                    (InlineKind::SoftBreak, Span::empty(base + at))
                };
                entries.push(Entry::Node(Inline::new(
                    kind,
                    Span::new(base + at, base + at + width),
                    Span::empty(base + at),
                    if hard { vec![marker] } else { Vec::new() },
                )));
                at += width;
                plain = at;
                continue;
            }
            let hit = match byte {
                b'\\' => inline_atom::escape(text, base, at),
                b'`' => inline_atom::code(text, base, at),
                b'<' => inline_atom::angle(text, base, at),
                b'$' => inline_atom::math(text, base, at),
                b'%' => inline_atom::comment(text, base, at),
                b'[' => inline_link::bracket(self, text, base, at, allow_links),
                b'!' => inline_link::bang(self, text, base, at, allow_links),
                b'#' => inline_atom::tag(text, base, at),
                b'*' | b'_' | b'~' | b'=' => delimiter(text, base, at),
                _ => None,
            };
            let hit = hit.or_else(|| {
                if allow_links && byte.is_ascii_alphanumeric() {
                    linkify(text, base, at, line_stop)
                } else {
                    None
                }
            });
            if let Some((entry, next)) = hit {
                flush(text, base, plain, at, &mut entries);
                entries.push(entry);
                at = next;
                plain = at;
            } else {
                at += char_len(text, at);
            }
        }
        flush(text, base, plain, text.len(), &mut entries);
        resolve(entries)
    }
}

fn linkify(text: &str, base: usize, at: usize, stop: usize) -> Option<(Entry, usize)> {
    let before = char_before(text, at);
    if before.is_some_and(|ch| ch.is_alphanumeric() || ch == '_') {
        return None;
    }
    let rest = text.get(at..stop)?;
    let (destination, email, used) = autolink::bare_link(rest)?;
    if used == 0 {
        return None;
    }
    let span = Span::new(base + at, base + at + used);
    let inline = Inline::new(
        InlineKind::Autolink { destination, email },
        span,
        span,
        Vec::new(),
    );
    Some((Entry::Node(inline), at + used))
}

fn flush(_text: &str, base: usize, from: usize, to: usize, entries: &mut Vec<Entry>) {
    if from >= to {
        return;
    }
    entries.push(Entry::Node(Inline::text(Span::new(base + from, base + to))));
}

fn to_inline(entry: Entry) -> Inline {
    match entry {
        Entry::Node(node) => node,
        Entry::Delim(delim) => Inline::text(delim.span),
    }
}

fn delimiter(text: &str, base: usize, at: usize) -> Option<(Entry, usize)> {
    let ch = byte_at(text, at)?;
    let mut count = 0;
    while byte_at(text, at + count) == Some(ch) {
        count += 1;
    }
    if matches!(ch, b'~' | b'=') && count != 2 {
        return None;
    }
    let end = at + count;
    let (can_open, can_close) = flanking(text, at, end, ch);
    if !can_open && !can_close {
        return None;
    }
    let span = Span::new(base + at, base + end);
    Some((
        Entry::Delim(Delim {
            ch,
            count,
            can_open,
            can_close,
            span,
        }),
        end,
    ))
}

fn flanking(text: &str, start: usize, end: usize, ch: u8) -> (bool, bool) {
    let before = char_before(text, start);
    let after = char_at(text, end);
    let before_ws = before.is_none_or(char::is_whitespace);
    let after_ws = after.is_none_or(char::is_whitespace);
    let before_punct = before.is_some_and(|value| value.is_ascii_punctuation());
    let after_punct = after.is_some_and(|value| value.is_ascii_punctuation());
    let left = !after_ws && (!after_punct || before_ws || before_punct);
    let right = !before_ws && (!before_punct || after_ws || after_punct);
    if ch == b'_' {
        (
            left && (!right || before_punct),
            right && (!left || after_punct),
        )
    } else {
        (left, right)
    }
}

pub(crate) fn matching_bracket(text: &str, from: usize) -> Option<usize> {
    let mut depth = 0;
    let mut at = from;
    while at < text.len() {
        match byte_at(text, at) {
            Some(b'\\') => at += 2,
            Some(b'`') => {
                let mut count = 0;
                while byte_at(text, at + count) == Some(b'`') {
                    count += 1;
                }
                let start = at + count;
                let stop = inline_atom::line_end(text, start);
                let close = inline_atom::find_byte(text, start, stop, b'`')?;
                at = close + count;
            }
            Some(b'[') => {
                depth += 1;
                at += 1;
            }
            Some(b']') => {
                if depth == 0 {
                    return Some(at);
                }
                depth -= 1;
                at += 1;
            }
            Some(_) => at += char_len(text, at),
            None => break,
        }
    }
    None
}

fn resolve(entries: Vec<Entry>) -> Vec<Inline> {
    let mut entries = entries;
    loop {
        let positions: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| matches!(entry, Entry::Delim(_)).then_some(index))
            .collect();
        let mut merged = false;
        for (at, closer_at) in positions.iter().copied().enumerate() {
            let Some(Entry::Delim(closer)) = entries.get(closer_at) else {
                continue;
            };
            let closer = *closer;
            if !closer.can_close || closer.count == 0 {
                continue;
            }
            let opener_at = positions.iter().take(at).rev().copied().find(|index| {
                matches!(
                    entries.get(*index),
                    Some(Entry::Delim(open))
                        if open.ch == closer.ch && open.can_open && open.count > 0
                )
            });
            let Some(opener_at) = opener_at else {
                continue;
            };
            let Some(Entry::Delim(opener)) = entries.get(opener_at) else {
                continue;
            };
            let opener = *opener;
            let use_len = if opener.count >= 2 && closer.count >= 2 {
                2
            } else {
                1
            };
            let kind = match closer.ch {
                b'~' => InlineKind::Strikethrough,
                b'=' => InlineKind::Highlight,
                _ if use_len == 2 => InlineKind::Strong,
                _ => InlineKind::Emphasis,
            };
            let open_marker = Span::new(opener.span.end - use_len, opener.span.end);
            let close_marker = Span::new(closer.span.start, closer.span.start + use_len);
            let node = Inline::new(
                kind,
                Span::new(open_marker.start, close_marker.end),
                Span::new(open_marker.end, close_marker.start),
                vec![open_marker, close_marker],
            );
            let children: Vec<Inline> = entries
                .drain(opener_at + 1..closer_at)
                .map(to_inline)
                .collect();
            let node = node.with_children(children);
            let mut replacement: Vec<Entry> = Vec::with_capacity(3);
            if opener.count > use_len {
                replacement.push(Entry::Delim(Delim {
                    ch: opener.ch,
                    count: opener.count - use_len,
                    can_open: opener.can_open,
                    can_close: opener.can_close,
                    span: Span::new(opener.span.start, opener.span.end - use_len),
                }));
            }
            replacement.push(Entry::Node(node));
            if closer.count > use_len {
                replacement.push(Entry::Delim(Delim {
                    ch: closer.ch,
                    count: closer.count - use_len,
                    can_open: closer.can_open,
                    can_close: closer.can_close,
                    span: Span::new(closer.span.start + use_len, closer.span.end),
                }));
            }
            entries.splice(opener_at..=opener_at + 1, replacement);
            merged = true;
            break;
        }
        if !merged {
            break;
        }
    }
    entries.into_iter().map(to_inline).collect()
}
