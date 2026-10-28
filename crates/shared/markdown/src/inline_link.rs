use crate::inline::{Inline, InlineKind, LinkTarget};
use crate::inline_atom::{find_byte, footnote_ref, line_end, skip_space, unescape, wiki_link};
use crate::inline_parse::{Ctx, Entry, matching_bracket};
use crate::span::Span;
use crate::text::{byte_at, char_before, char_len};

struct Destination {
    target: LinkTarget,
    url: Span,
    close: usize,
}

fn inline_destination(text: &str, base: usize, from: usize) -> Option<Destination> {
    let mut at = skip_space(text, from);
    let url_start = at;
    let destination = if byte_at(text, at) == Some(b'<') {
        let close = find_byte(text, at + 1, line_end(text, at + 1), b'>')?;
        let value = text.get(at + 1..close)?;
        if value.contains('<') {
            return None;
        }
        at = close + 1;
        value.to_owned()
    } else {
        let start = at;
        let mut depth = 0;
        while at < text.len() {
            match byte_at(text, at) {
                Some(b'\\') => at += 2,
                Some(b'(') => {
                    depth += 1;
                    at += 1;
                }
                Some(b')') => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    at += 1;
                }
                Some(b' ' | b'\t' | b'\n' | b'\r') | None => break,
                Some(_) => at += char_len(text, at),
            }
        }
        text.get(start..at)?.to_owned()
    };
    let url = Span::new(base + url_start, base + at);
    let spaced = skip_space(text, at);
    let quoted = spaced > at && matches!(byte_at(text, spaced), Some(b'"' | b'\'' | b'('));
    let (title, after) = if quoted {
        let open = byte_at(text, spaced)?;
        let close_char = if open == b'(' { b')' } else { open };
        let mut scan = spaced + 1;
        while scan < text.len() {
            match byte_at(text, scan) {
                Some(b'\\') => scan += 2,
                Some(byte) if byte == close_char => break,
                Some(_) => scan += char_len(text, scan),
                None => break,
            }
        }
        if byte_at(text, scan) != Some(close_char) {
            return None;
        }
        (
            Some(text.get(spaced + 1..scan)?.to_owned()),
            skip_space(text, scan + 1),
        )
    } else {
        (None, spaced)
    };
    if byte_at(text, after) != Some(b')') {
        return None;
    }
    Some(Destination {
        target: LinkTarget::new(unescape(&destination), title),
        url,
        close: after,
    })
}

fn link(ctx: &Ctx<'_>, text: &str, base: usize, at: usize, image: bool) -> Option<(Entry, usize)> {
    let bracket_at = at + usize::from(image);
    if byte_at(text, bracket_at) != Some(b'[') {
        return None;
    }
    let label_start = bracket_at + 1;
    let label_end = matching_bracket(text, label_start)?;
    let label_span = Span::new(base + label_start, base + label_end);
    let after = label_end + 1;
    let open_marker = Span::new(base + at, base + label_start);
    let children = || ctx.sequence(label_span, false);
    match byte_at(text, after) {
        Some(b'(') => {
            let destination = inline_destination(text, base, after + 1)?;
            let markers = vec![
                open_marker,
                Span::new(base + label_end, base + after + 1),
                destination.url,
                Span::new(base + destination.close, base + destination.close + 1),
            ];
            let kind = if image {
                InlineKind::Image(Box::new(destination.target))
            } else {
                InlineKind::Link(Box::new(destination.target))
            };
            let node = Inline::new(
                kind,
                Span::new(base + at, base + destination.close + 1),
                label_span,
                markers,
            )
            .with_children(children());
            Some((Entry::Node(node), destination.close + 1))
        }
        Some(b'[') => {
            let reference_end = matching_bracket(text, after + 1)?;
            let reference = text.get(after + 1..reference_end)?;
            let label = if reference.trim().is_empty() {
                text.get(label_start..label_end)?
            } else {
                reference
            };
            let target = ctx.definitions.get(label)?.clone();
            let markers = vec![
                open_marker,
                Span::new(base + label_end, base + reference_end + 1),
            ];
            let kind = if image {
                InlineKind::Image(Box::new(target))
            } else {
                InlineKind::Link(Box::new(target))
            };
            let node = Inline::new(
                kind,
                Span::new(base + at, base + reference_end + 1),
                label_span,
                markers,
            )
            .with_children(children());
            Some((Entry::Node(node), reference_end + 1))
        }
        _ => {
            let label = text.get(label_start..label_end)?;
            let target = ctx.definitions.get(label)?.clone();
            let markers = vec![
                open_marker,
                Span::new(base + label_end, base + label_end + 1),
            ];
            let kind = if image {
                InlineKind::Image(Box::new(target))
            } else {
                InlineKind::Link(Box::new(target))
            };
            let node = Inline::new(
                kind,
                Span::new(base + at, base + label_end + 1),
                label_span,
                markers,
            )
            .with_children(children());
            Some((Entry::Node(node), label_end + 1))
        }
    }
}

pub(crate) fn bracket(
    ctx: &Ctx<'_>,
    text: &str,
    base: usize,
    at: usize,
    allow_links: bool,
) -> Option<(Entry, usize)> {
    if byte_at(text, at + 1) == Some(b'[')
        && let Some(hit) = wiki_link(text, base, at, false)
    {
        return Some(hit);
    }
    if byte_at(text, at + 1) == Some(b'^')
        && let Some(hit) = footnote_ref(text, base, at)
    {
        return Some(hit);
    }
    if !allow_links {
        return None;
    }
    link(ctx, text, base, at, false)
}

pub(crate) fn bang(
    ctx: &Ctx<'_>,
    text: &str,
    base: usize,
    at: usize,
    allow_links: bool,
) -> Option<(Entry, usize)> {
    if byte_at(text, at + 1) == Some(b'[') && byte_at(text, at + 2) == Some(b'[') {
        let before = char_before(text, at);
        if before.is_some_and(|ch| ch.is_alphanumeric() || ch == '_') {
            return None;
        }
        return wiki_link(text, base, at, true);
    }
    if !allow_links {
        return None;
    }
    link(ctx, text, base, at, true)
}
