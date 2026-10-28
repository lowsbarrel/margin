use crate::block::{Block, BlockKind};
use crate::callout::{CalloutKind, Fold, capitalize_type};
use crate::link_ref::LinkDefinitions;
use crate::parse::parse_blocks;
use crate::span::Span;
use crate::text::{Line, is_blank, leading_spaces};

fn quote_prefix(text: &str) -> Option<(usize, usize)> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?;
    if !rest.starts_with('>') {
        return None;
    }
    let after = rest.get(1..)?;
    let space = usize::from(after.starts_with([' ', '\t']));
    Some((indent, indent + 1 + space))
}

#[must_use]
pub(crate) fn is_blockquote(text: &str) -> bool {
    quote_prefix(text).is_some()
}

struct CalloutHeader {
    kind: CalloutKind,
    fold: Fold,
    title: String,
    len: usize,
}

fn callout_header(text: &str) -> Option<CalloutHeader> {
    let rest = text.strip_prefix("[!")?;
    let mut name_end = 0;
    for (offset, ch) in rest.char_indices() {
        if ch.is_ascii_alphabetic() {
            name_end = offset + 1;
        } else {
            break;
        }
    }
    if name_end == 0 {
        return None;
    }
    let name = rest.get(..name_end)?;
    let after = rest.get(name_end..)?.strip_prefix(']')?;
    let (fold, after) = match after.chars().next() {
        Some('+') => (Fold::Expanded, after.get(1..).unwrap_or_default()),
        Some('-') => (Fold::Collapsed, after.get(1..).unwrap_or_default()),
        _ => (Fold::None, after),
    };
    let title = after.trim();
    let fold_len = usize::from(fold != Fold::None);
    let title = if title.is_empty() {
        capitalize_type(name)
    } else {
        title.to_owned()
    };
    Some(CalloutHeader {
        kind: CalloutKind::from_type(name),
        fold,
        title,
        len: 2 + name_end + 1 + fold_len,
    })
}

pub(crate) fn content_span(body: &[Line], fallback: usize) -> Span {
    match (body.first(), body.last()) {
        (Some(first), Some(last)) => Span::new(first.start, last.end),
        _ => Span::empty(fallback),
    }
}

pub(crate) fn blockquote(
    source: &str,
    lines: &[Line],
    index: usize,
    definitions: &mut LinkDefinitions,
) -> Option<(Block, usize)> {
    let first = lines.get(index)?;
    quote_prefix(first.text(source))?;
    let mut content: Vec<Line> = Vec::new();
    let mut markers: Vec<Span> = Vec::new();
    let mut at = index;
    let mut end = first.end;
    while let Some(line) = lines.get(at) {
        let text = line.text(source);
        if let Some((marker_start, content_start)) = quote_prefix(text) {
            markers.push(Span::new(
                line.start + marker_start,
                line.start + content_start,
            ));
            content.push(Line {
                start: line.start + content_start,
                end: line.end,
            });
            end = line.end;
            at += 1;
            continue;
        }
        if is_blank(text) && next_has_prefix(source, lines, at) {
            content.push(Line {
                start: line.start,
                end: line.start,
            });
            at += 1;
            continue;
        }
        break;
    }
    let span = Span::new(first.start, end);
    let header = content
        .first()
        .map_or("", |line| line.text(source))
        .to_owned();
    if let Some(header) = callout_header(&header) {
        let body = content.get(1..).unwrap_or_default().to_vec();
        let mut callout_markers = markers;
        if let Some(line) = content.first() {
            callout_markers.push(Span::new(line.start, line.start + header.len));
        }
        let children = parse_blocks(source, &body, false, definitions);
        let block = Block::new(
            BlockKind::Callout {
                kind: header.kind,
                fold: header.fold,
                title: header.title,
            },
            span,
        )
        .with_content(content_span(&body, span.end))
        .with_markers(callout_markers)
        .with_children(children);
        return Some((block, at));
    }
    let children = parse_blocks(source, &content, false, definitions);
    let block = Block::new(BlockKind::BlockQuote, span)
        .with_content(content_span(&content, span.end))
        .with_markers(markers)
        .with_children(children);
    Some((block, at))
}

fn next_has_prefix(source: &str, lines: &[Line], at: usize) -> bool {
    let mut probe = at + 1;
    while let Some(next) = lines.get(probe) {
        let text = next.text(source);
        if quote_prefix(text).is_some() {
            return true;
        }
        if !is_blank(text) {
            return false;
        }
        probe += 1;
    }
    false
}

fn colon_header(text: &str) -> Option<(String, usize, String)> {
    let colons = text.chars().take_while(|ch| *ch == ':').count();
    if colons < 3 {
        return None;
    }
    let after = text.get(colons..)?;
    let spaced = after.trim_start_matches([' ', '\t']);
    let mut name_end = 0;
    for (offset, ch) in spaced.char_indices() {
        if ch.is_alphanumeric() || matches!(ch, '_' | '-') {
            name_end = offset + ch.len_utf8();
        } else {
            break;
        }
    }
    let name = spaced.get(..name_end)?;
    if !name.is_empty() && !name.starts_with(|first: char| first.is_ascii_alphabetic()) {
        return None;
    }
    let title = spaced.get(name_end..).unwrap_or_default().trim().to_owned();
    let marker_len = colons + (after.len() - spaced.len()) + name_end;
    Some((name.to_owned(), marker_len, title))
}

#[must_use]
pub(crate) fn is_colon_callout(text: &str) -> bool {
    colon_header(text).is_some()
}

pub(crate) fn colon_callout(
    source: &str,
    lines: &[Line],
    index: usize,
    definitions: &mut LinkDefinitions,
) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    let (name, marker_len, title) = colon_header(line.text(source))?;
    let mut at = index + 1;
    let mut close = None;
    while let Some(next) = lines.get(at) {
        let text = next.text(source);
        let trimmed = text.trim_end();
        if trimmed.starts_with(":::") && trimmed.get(3..).unwrap_or_default().trim().is_empty() {
            close = Some(next);
            at += 1;
            break;
        }
        at += 1;
    }
    let limit = if close.is_some() { at - 1 } else { lines.len() };
    let body: Vec<Line> = lines.get(index + 1..limit).unwrap_or_default().to_vec();
    let mut markers = vec![Span::new(line.start, line.start + marker_len)];
    if let Some(close) = close {
        markers.push(Span::new(close.start, close.start + 3));
    }
    let span_end = close.map_or_else(
        || lines.last().map_or(line.end, |last| last.end),
        |close| close.end,
    );
    let kind = CalloutKind::from_type(&name);
    let title = if title.is_empty() {
        if name.is_empty() {
            kind.title().to_owned()
        } else {
            capitalize_type(&name)
        }
    } else {
        title
    };
    let children = parse_blocks(source, &body, false, definitions);
    let block = Block::new(
        BlockKind::ColonCallout { kind, name, title },
        Span::new(line.start, span_end),
    )
    .with_content(content_span(&body, span_end))
    .with_markers(markers)
    .with_children(children);
    Some((block, at))
}
