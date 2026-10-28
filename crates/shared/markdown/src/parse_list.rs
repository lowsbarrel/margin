use crate::block::{Block, BlockKind};
use crate::link_ref::LinkDefinitions;
use crate::parse::parse_blocks;
use crate::parse_container::content_span;
use crate::span::Span;
use crate::text::{Line, byte_at, indent_of, is_blank, leading_spaces, strip_columns};

#[derive(Clone, Copy)]
struct Item {
    ordered: bool,
    start: u64,
    bullet: u8,
    delim: u8,
    marker_start: usize,
    content_start: usize,
}

fn list_marker(text: &str) -> Option<Item> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?;
    let first = byte_at(rest, 0)?;
    if matches!(first, b'-' | b'+' | b'*') {
        let after = rest.get(1..)?;
        if !after.is_empty() && !after.starts_with([' ', '\t']) {
            return None;
        }
        let spaces = if after.is_empty() {
            1
        } else {
            (after.len() - after.trim_start_matches([' ', '\t']).len()).min(4)
        };
        return Some(Item {
            ordered: false,
            start: 1,
            bullet: first,
            delim: first,
            marker_start: indent,
            content_start: indent + 1 + spaces,
        });
    }
    if !first.is_ascii_digit() {
        return None;
    }
    let mut digits = 0;
    while byte_at(rest, digits).is_some_and(|byte| byte.is_ascii_digit()) {
        digits += 1;
    }
    if digits == 0 || digits > 9 {
        return None;
    }
    let delim = byte_at(rest, digits)?;
    if delim != b'.' && delim != b')' {
        return None;
    }
    let after = rest.get(digits + 1..)?;
    if !after.is_empty() && !after.starts_with([' ', '\t']) {
        return None;
    }
    let start = rest.get(..digits)?.parse::<u64>().ok()?;
    let spaces = if after.is_empty() {
        1
    } else {
        (after.len() - after.trim_start_matches([' ', '\t']).len()).min(4)
    };
    Some(Item {
        ordered: true,
        start,
        bullet: 0,
        delim,
        marker_start: indent,
        content_start: indent + digits + 1 + spaces,
    })
}

#[must_use]
pub(crate) fn is_list_item(text: &str) -> bool {
    list_marker(text).is_some()
}

fn task_box(text: &str) -> Option<bool> {
    let inner = text.strip_prefix('[')?.chars().next()?;
    let checked = match inner {
        ' ' => false,
        'x' | 'X' => true,
        _ => return None,
    };
    let rest = text.get(3..)?;
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    Some(checked)
}

pub(crate) fn list(
    source: &str,
    lines: &[Line],
    index: usize,
    definitions: &mut LinkDefinitions,
) -> Option<(Block, usize)> {
    let first_line = lines.get(index)?;
    let first = list_marker(first_line.text(source))?;
    let mut items: Vec<Block> = Vec::new();
    let mut at = index;
    let mut list_end = first_line.end;
    while let Some(line) = lines.get(at) {
        let text = line.text(source);
        let Some(marker) = list_marker(text) else {
            break;
        };
        if marker.ordered != first.ordered {
            break;
        }
        if marker.ordered {
            if marker.delim != first.delim {
                break;
            }
        } else if marker.bullet != first.bullet {
            break;
        }
        let column = marker.content_start;
        let start = marker.content_start.min(text.len());
        let mut body: Vec<Line> = vec![Line {
            start: line.start + start,
            end: line.end,
        }];
        let mut markers = vec![Span::new(
            line.start + marker.marker_start,
            line.start + marker.content_start.min(text.len()),
        )];
        let mut item_end = line.end;
        let mut loose = false;
        at += 1;
        while let Some(next) = lines.get(at) {
            let next_text = next.text(source);
            if is_blank(next_text) {
                let Some((probe, later)) = next_content(source, lines, at) else {
                    break;
                };
                if indent_of(later) >= column {
                    loose = true;
                    body.push(Line {
                        start: next.start,
                        end: next.start,
                    });
                    item_end = next.end;
                    at += 1;
                    continue;
                }
                if same_kind(list_marker(later), &first) {
                    loose = true;
                    at = probe;
                }
                break;
            }
            if indent_of(next_text) < column {
                break;
            }
            let strip = strip_columns(next_text, column);
            body.push(Line {
                start: next.start + strip,
                end: next.end,
            });
            item_end = next.end;
            at += 1;
        }
        let mut task = None;
        if let Some(entry) = body.first_mut()
            && let Some(checked) = task_box(entry.text(source))
        {
            task = Some(checked);
            markers.push(Span::new(entry.start, entry.start + 3));
            let text = entry.text(source);
            let strip = 3 + usize::from(
                text.get(3..)
                    .is_some_and(|rest| rest.starts_with([' ', '\t'])),
            );
            entry.start += strip;
        }
        let children = parse_blocks(source, &body, false, definitions);
        let item = Block::new(
            BlockKind::ListItem { task, loose },
            Span::new(line.start, item_end),
        )
        .with_content(content_span(&body, item_end))
        .with_markers(markers)
        .with_children(children);
        items.push(item);
        list_end = item_end;
    }
    if items.is_empty() {
        return None;
    }
    let kind = if first.ordered {
        BlockKind::OrderedList { start: first.start }
    } else {
        BlockKind::BulletList
    };
    let block = Block::new(kind, Span::new(first_line.start, list_end)).with_children(items);
    Some((block, at))
}

fn next_content<'a>(source: &'a str, lines: &[Line], at: usize) -> Option<(usize, &'a str)> {
    let mut probe = at + 1;
    while let Some(next) = lines.get(probe) {
        let text = next.text(source);
        if !is_blank(text) {
            return Some((probe, text));
        }
        probe += 1;
    }
    None
}

fn same_kind(candidate: Option<Item>, first: &Item) -> bool {
    let Some(candidate) = candidate else {
        return false;
    };
    candidate.ordered == first.ordered
        && if first.ordered {
            candidate.delim == first.delim
        } else {
            candidate.bullet == first.bullet
        }
}

fn footnote_header(text: &str) -> Option<(String, usize, usize)> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?.strip_prefix("[^")?;
    let close = rest.find("]:")?;
    let id = rest.get(..close)?;
    if id.is_empty() || id.chars().any(|ch| ch.is_whitespace() || ch == ']') {
        return None;
    }
    let marker_end = indent + 2 + close + 2;
    let tail = text.get(marker_end..)?;
    let space = tail.len() - tail.trim_start_matches([' ', '\t']).len();
    Some((id.to_lowercase(), marker_end, marker_end + space))
}

#[must_use]
pub(crate) fn is_footnote_definition(text: &str) -> bool {
    footnote_header(text).is_some()
}

pub(crate) fn footnote_definition(
    source: &str,
    lines: &[Line],
    index: usize,
    definitions: &mut LinkDefinitions,
) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    let text = line.text(source);
    let (id, marker_end, body_start) = footnote_header(text)?;
    let mut body: Vec<Line> = vec![Line {
        start: line.start + body_start.min(text.len()),
        end: line.end,
    }];
    let mut at = index + 1;
    let mut last_end = line.end;
    let mut pending_blank = 0;
    while let Some(next) = lines.get(at) {
        let next_text = next.text(source);
        if is_blank(next_text) {
            pending_blank += 1;
            at += 1;
            continue;
        }
        if indent_of(next_text) < 4 {
            break;
        }
        pending_blank = 0;
        let strip = strip_columns(next_text, 4);
        body.push(Line {
            start: next.start + strip,
            end: next.end,
        });
        last_end = next.end;
        at += 1;
    }
    let at = at.saturating_sub(pending_blank);
    let span = Span::new(line.start, last_end);
    let children = parse_blocks(source, &body, false, definitions);
    let block = Block::new(BlockKind::FootnoteDefinition { id }, span)
        .with_content(content_span(&body, span.end))
        .with_markers(vec![Span::new(
            line.start + leading_spaces(text),
            line.start + marker_end,
        )])
        .with_children(children);
    Some((block, at))
}
