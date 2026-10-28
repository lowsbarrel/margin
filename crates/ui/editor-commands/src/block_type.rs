use margin_markdown::{Block, BlockKind, Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::mapping::{self, Assoc};
use crate::text::{self, Line};
use crate::{prefix, tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockType {
    Text,
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,
    BulletList,
    OrderedList,
    TaskList,
    Quote,
    CodeBlock,
}

pub const BUBBLE_BLOCK_TYPES: [BlockType; 9] = [
    BlockType::Text,
    BlockType::Heading1,
    BlockType::Heading2,
    BlockType::Heading3,
    BlockType::BulletList,
    BlockType::OrderedList,
    BlockType::TaskList,
    BlockType::Quote,
    BlockType::CodeBlock,
];

const HEADINGS: [BlockType; 6] = [
    BlockType::Heading1,
    BlockType::Heading2,
    BlockType::Heading3,
    BlockType::Heading4,
    BlockType::Heading5,
    BlockType::Heading6,
];

impl BlockType {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Text | Self::CodeBlock => "",
            Self::Heading1 => "# ",
            Self::Heading2 => "## ",
            Self::Heading3 => "### ",
            Self::Heading4 => "#### ",
            Self::Heading5 => "##### ",
            Self::Heading6 => "###### ",
            Self::BulletList => "- ",
            Self::OrderedList => "1. ",
            Self::TaskList => "- [ ] ",
            Self::Quote => "> ",
        }
    }

    const fn nests(self) -> bool {
        matches!(self, Self::BulletList | Self::OrderedList | Self::TaskList)
    }
}

fn strip_block(line: &str) -> String {
    let mut rest = line;
    if let Some((_, length)) = prefix::heading(rest) {
        rest = rest.get(length..).unwrap_or_default();
    }
    if let Some(length) = prefix::quote(rest) {
        rest = rest.get(length..).unwrap_or_default();
    }
    match prefix::spaced_marker(rest) {
        Some(found) => format!(
            "{}{}",
            found.indent,
            rest.get(found.len()..).unwrap_or_default()
        ),
        None => rest.to_owned(),
    }
}

fn block_line(block: BlockType, line: &str) -> String {
    let body = strip_block(line);
    let prefix = block.prefix();
    if prefix.is_empty() {
        return body;
    }
    let indent = if block.nests() {
        text::leading_space(&body).len()
    } else {
        0
    };
    format!(
        "{}{prefix}{}",
        body.get(..indent).unwrap_or_default(),
        body.get(indent..).unwrap_or_default()
    )
}

fn is_fence_line(line: &str) -> bool {
    let rest = line.trim_start_matches([' ', '\t']);
    rest.starts_with("```") || rest.starts_with("~~~")
}

struct Unwrap {
    edits: Vec<TextEdit>,
    open: usize,
    close: Option<usize>,
    content_end: usize,
}

fn unwrap_fence(document: &Document, fence: Span) -> Unwrap {
    let open = Line::at(document, fence.start);
    let close = Line::at(document, fence.end.saturating_sub(1).max(open.start + 1));
    let closed = close.index > open.index && is_fence_line(close.text);
    let open_end = (open.end + 1).min(document.text().len());
    let mut edits = vec![TextEdit::new(Span::new(open.start, open_end), "")];
    if closed {
        edits.push(TextEdit::new(Span::new(close.start - 1, close.end), ""));
    }
    Unwrap {
        edits,
        open: open.start,
        close: closed.then_some(close.start),
        content_end: if closed { close.start - 1 } else { fence.end },
    }
}

#[must_use]
pub fn set_block(
    document: &Document,
    selections: &[Selection],
    primary: usize,
    block: BlockType,
) -> Option<Change> {
    let range = *selections.get(primary).or_else(|| selections.last())?;
    let lines: Vec<Line<'_>> = text::selected_lines(document, range)
        .map(|index| Line::numbered(document, index))
        .collect();
    let (first, last) = (lines.first()?, lines.last()?);
    let fence =
        tree::fenced_code_at(document, range.head).map(|code| unwrap_fence(document, code.span));
    let mut edits = Vec::new();
    if fence.is_none() && block == BlockType::CodeBlock {
        edits.push(TextEdit::new(Span::empty(first.start), "```\n"));
        edits.push(TextEdit::new(Span::empty(last.end), "\n```"));
    }
    if let Some(fence) = &fence {
        edits.extend(fence.edits.iter().cloned());
    }
    if block != BlockType::CodeBlock {
        for line in &lines {
            let fence_line = fence
                .as_ref()
                .is_some_and(|fence| line.start == fence.open || Some(line.start) == fence.close);
            let next = block_line(block, line.text);
            if !fence_line && let Some(edit) = mapping::diff(line.text, &next) {
                edits.push(TextEdit::new(
                    Span::new(line.start + edit.span.start, line.start + edit.span.end),
                    edit.insert,
                ));
            }
        }
    }
    if edits.is_empty() {
        return None;
    }
    let edits = mapping::sorted(edits);
    let selection = match &fence {
        Some(fence) => Some(Selection::new(
            mapping::map_pos(&edits, fence.open, Assoc::Before),
            mapping::map_pos(&edits, fence.content_end, Assoc::Before),
        )),
        None if block == BlockType::CodeBlock => Some(Selection::new(
            first.start,
            mapping::map_pos(&edits, last.end, Assoc::After),
        )),
        None => None,
    };
    let change = Change::new(edits);
    Some(match selection {
        Some(selection) => change.with_selections(vec![selection]),
        None => change,
    })
}

fn covering(document: &Document, selection: Selection) -> Vec<&Block> {
    let (from, to) = (selection.start(), selection.end());
    let mut names = tree::ancestors(document, from);
    if from != to {
        names.extend(tree::ancestors(document, to));
    }
    names.retain(|block| block.span.start <= from && block.span.end >= to);
    names
}

#[must_use]
pub fn block_type(document: &Document, selection: Selection) -> BlockType {
    if tree::fenced_code_at(document, selection.head).is_some() {
        return BlockType::CodeBlock;
    }
    let names = covering(document, selection);
    let level = names.iter().filter_map(|block| block.heading_level()).min();
    if let Some(level) = level {
        let index = usize::from(level.clamp(1, 6)) - 1;
        return HEADINGS.get(index).copied().unwrap_or(BlockType::Text);
    }
    let has = |test: fn(&BlockKind) -> bool| names.iter().any(|block| test(&block.kind));
    if has(|kind| matches!(kind, BlockKind::ListItem { task: Some(_), .. })) {
        BlockType::TaskList
    } else if has(|kind| matches!(kind, BlockKind::BulletList)) {
        BlockType::BulletList
    } else if has(|kind| matches!(kind, BlockKind::OrderedList { .. })) {
        BlockType::OrderedList
    } else if has(tree::is_quote) {
        BlockType::Quote
    } else {
        BlockType::Text
    }
}

#[must_use]
pub fn inside_code(document: &Document, selection: Selection) -> bool {
    if covering(document, selection)
        .iter()
        .any(|block| matches!(block.kind, BlockKind::CodeBlock { fenced: false, .. }))
    {
        return true;
    }
    tree::fenced_code_at(document, selection.head).is_some_and(|fence| {
        selection.start() > fence.span.start || selection.end() < fence.span.end
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{primary, run};

    fn set(source: &str, block: BlockType) -> Option<String> {
        run(source, |d, s| set_block(d, s, primary(s), block))
    }

    #[test]
    fn converts_lines_between_block_types() {
        assert_eq!(set("## ti|tle", BlockType::Text).as_deref(), Some("ti|tle"));
        assert_eq!(
            set("> - it|em", BlockType::Heading1).as_deref(),
            Some("# it|em")
        );
        assert_eq!(
            set("  - it|em", BlockType::TaskList).as_deref(),
            Some("  - [ ] it|em")
        );
        assert_eq!(
            set("«a\nb»", BlockType::Quote).as_deref(),
            Some("> «a\n> b»")
        );
        assert_eq!(set("te|xt", BlockType::Text), None);
    }

    #[test]
    fn wraps_and_unwraps_code_fences() {
        assert_eq!(
            set("«a\nb»", BlockType::CodeBlock).as_deref(),
            Some("«```\na\nb\n```»")
        );
        assert_eq!(
            set("```\nco|de\n```\nafter", BlockType::CodeBlock).as_deref(),
            Some("«code»\nafter")
        );
        assert_eq!(
            set("```\nco|de\n```", BlockType::BulletList).as_deref(),
            Some("«- code»")
        );
    }

    #[test]
    fn reports_the_current_block_type() {
        let document =
            Document::parse("# Title\n\n- [ ] task\n\n> quote\n\n1. one\n\n```\ncode\n```\n");
        assert_eq!(
            block_type(&document, Selection::cursor(3)),
            BlockType::Heading1
        );
        assert_eq!(
            block_type(&document, Selection::cursor(16)),
            BlockType::TaskList
        );
        assert_eq!(
            block_type(&document, Selection::cursor(24)),
            BlockType::Quote
        );
        assert_eq!(
            block_type(&document, Selection::cursor(32)),
            BlockType::OrderedList
        );
        assert_eq!(
            block_type(&document, Selection::cursor(40)),
            BlockType::CodeBlock
        );
        assert!(inside_code(&document, Selection::cursor(40)));
        assert!(!inside_code(&document, Selection::cursor(3)));
    }
}
