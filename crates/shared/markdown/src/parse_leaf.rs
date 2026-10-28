use crate::block::{Block, BlockKind, HeadingStyle};
use crate::frontmatter;
use crate::span::Span;
use crate::text::{Line, byte_at, leading_spaces};

pub(crate) fn frontmatter_block(source: &str, lines: &[Line]) -> Option<(Block, usize)> {
    let (model, next) = frontmatter::parse(source, lines)?;
    let span = model.span;
    Some((Block::new(BlockKind::Frontmatter(model), span), next))
}

fn thematic_marker(text: &str) -> Option<usize> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?;
    let mut marker = None;
    let mut count = 0;
    for ch in rest.chars() {
        if ch == ' ' || ch == '\t' {
            continue;
        }
        match marker {
            None if matches!(ch, '-' | '*' | '_') => {
                marker = Some(ch);
                count = 1;
            }
            Some(value) if value == ch => count += 1,
            _ => return None,
        }
    }
    (count >= 3).then_some(indent)
}

#[must_use]
pub(crate) fn is_thematic_break(text: &str) -> bool {
    thematic_marker(text).is_some()
}

pub(crate) fn thematic_break(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    thematic_marker(line.text(source))?;
    let block = Block::new(BlockKind::ThematicBreak, line.span()).with_markers(vec![line.span()]);
    Some((block, index + 1))
}

fn split_closing(body: &str) -> (&str, Option<&str>) {
    let trimmed = body.trim_end_matches([' ', '\t']);
    let hashes = trimmed.len() - trimmed.trim_end_matches('#').len();
    if hashes == 0 {
        return (trimmed, None);
    }
    let head = trimmed.get(..trimmed.len() - hashes).unwrap_or_default();
    if head.is_empty() || head.ends_with([' ', '\t']) {
        (
            head.trim_end_matches([' ', '\t']),
            trimmed.get(trimmed.len() - hashes..),
        )
    } else {
        (trimmed, None)
    }
}

fn atx_parts(text: &str) -> Option<(u8, usize, usize)> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?;
    let hashes = rest.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let after = rest.get(hashes..)?;
    if !after.is_empty() && !after.starts_with([' ', '\t']) {
        return None;
    }
    let spaces = after.len() - after.trim_start_matches([' ', '\t']).len();
    Some((u8::try_from(hashes).ok()?, indent + hashes + spaces, indent))
}

#[must_use]
pub(crate) fn is_atx_heading(text: &str) -> bool {
    atx_parts(text).is_some()
}

pub(crate) fn atx_heading(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    let text = line.text(source);
    let (level, content_start, indent) = atx_parts(text)?;
    let body = text.get(content_start..)?;
    let (content, closing) = split_closing(body);
    let mut markers = vec![Span::new(line.start + indent, line.start + content_start)];
    if closing.is_some() {
        markers.push(Span::new(
            line.start + content_start + content.len(),
            line.end,
        ));
    }
    let block = Block::new(
        BlockKind::Heading {
            level,
            style: HeadingStyle::Atx,
        },
        line.span(),
    )
    .with_content(Span::new(
        line.start + content_start,
        line.start + content_start + content.len(),
    ))
    .with_markers(markers);
    Some((block, index + 1))
}

struct Fence {
    ch: u8,
    count: usize,
    indent: usize,
    info: String,
}

fn fence_open(text: &str) -> Option<Fence> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?;
    let ch = byte_at(rest, 0)?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let mut count = 0;
    while byte_at(rest, count) == Some(ch) {
        count += 1;
    }
    if count < 3 {
        return None;
    }
    let info = rest.get(count..)?.trim().to_owned();
    if ch == b'`' && info.contains('`') {
        return None;
    }
    Some(Fence {
        ch,
        count,
        indent,
        info,
    })
}

fn fence_close(text: &str, ch: u8, count: usize) -> Option<usize> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?;
    let mut run = 0;
    while byte_at(rest, run) == Some(ch) {
        run += 1;
    }
    if run < count || !rest.get(run..).unwrap_or_default().trim().is_empty() {
        return None;
    }
    Some(indent)
}

#[must_use]
pub(crate) fn is_fence(text: &str) -> bool {
    fence_open(text).is_some()
}

pub(crate) fn fence(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    let fence = fence_open(line.text(source))?;
    let mut markers = vec![Span::new(line.start + fence.indent, line.end)];
    let mut at = index + 1;
    let mut close = None;
    while let Some(next) = lines.get(at) {
        if fence_close(next.text(source), fence.ch, fence.count).is_some() {
            close = Some(next);
            at += 1;
            break;
        }
        at += 1;
    }
    let body_start = (line.end + 1).min(source.len());
    let (body_end, span_end) = if let Some(close) = close {
        markers.push(Span::new(
            close.start + leading_spaces(close.text(source)),
            close.end,
        ));
        (close.start, close.end)
    } else {
        let last = lines
            .get(at.saturating_sub(1))
            .map_or(line.end, |line| line.end);
        (last, last)
    };
    let body_end = body_end.max(body_start);
    let block = Block::new(
        BlockKind::CodeBlock {
            info: fence.info,
            fenced: true,
        },
        Span::new(line.start, span_end),
    )
    .with_content(Span::new(body_start, body_end))
    .with_markers(markers);
    Some((block, at))
}

fn math_fence(text: &str) -> Option<usize> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?;
    if !rest.starts_with("$$") {
        return None;
    }
    Some(indent)
}

#[must_use]
pub(crate) fn is_block_math(text: &str) -> bool {
    math_fence(text).is_some()
}

pub(crate) fn block_math(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    let text = line.text(source);
    let indent = math_fence(text)?;
    let rest = text.get(indent..)?;
    let tail = rest.get(2..)?;
    if let Some(offset) = tail.find("$$") {
        let content = tail.get(..offset)?;
        let after = tail.get(offset + 2..)?;
        if !content.is_empty() && after.trim().is_empty() {
            let start = line.start + indent + 2;
            let markers = vec![
                Span::new(line.start + indent, start),
                Span::new(start + offset, start + offset + 2),
            ];
            let block = Block::new(BlockKind::MathBlock { single_line: true }, line.span())
                .with_content(Span::new(start, start + offset))
                .with_markers(markers);
            return Some((block, index + 1));
        }
    }
    if rest.trim_end() != "$$" {
        return None;
    }
    let mut at = index + 1;
    let mut close = None;
    while let Some(next) = lines.get(at) {
        if next.text(source).trim_end() == "$$" {
            close = Some(next);
            at += 1;
            break;
        }
        at += 1;
    }
    let body_start = (line.end + 1).min(source.len());
    let mut markers = vec![Span::new(line.start + indent, line.start + indent + 2)];
    let (body_end, span_end) = if let Some(close) = close {
        markers.push(Span::new(close.start, close.start + 2));
        (close.start, close.end)
    } else {
        let last = lines
            .get(at.saturating_sub(1))
            .map_or(line.end, |line| line.end);
        (last, last)
    };
    let body_end = body_end.max(body_start);
    let block = Block::new(
        BlockKind::MathBlock { single_line: false },
        Span::new(line.start, span_end),
    )
    .with_content(Span::new(body_start, body_end))
    .with_markers(markers);
    Some((block, at))
}

#[must_use]
pub(crate) fn opens_comment(text: &str) -> bool {
    text.starts_with("%%") && !text.get(2..).unwrap_or_default().contains("%%")
}

#[must_use]
pub(crate) fn is_comment_block(text: &str) -> bool {
    opens_comment(text)
}

pub(crate) fn comment_block(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    if !opens_comment(line.text(source)) {
        return None;
    }
    let mut at = index + 1;
    let mut close = None;
    while let Some(next) = lines.get(at) {
        if let Some(offset) = next.text(source).find("%%") {
            close = Some((next, offset));
            at += 1;
            break;
        }
        at += 1;
    }
    let body_start = (line.start + 2).min(source.len());
    let mut markers = vec![Span::new(line.start, line.start + 2)];
    let (body_end, span_end) = if let Some((next, offset)) = close {
        markers.push(Span::new(next.start + offset, next.start + offset + 2));
        (next.start + offset, next.end)
    } else {
        let last = lines
            .get(at.saturating_sub(1))
            .map_or(line.end, |line| line.end);
        (last, last)
    };
    let body_end = body_end.max(body_start);
    let block = Block::new(BlockKind::Comment, Span::new(line.start, span_end))
        .with_content(Span::new(body_start, body_end))
        .with_markers(markers);
    Some((block, at))
}
