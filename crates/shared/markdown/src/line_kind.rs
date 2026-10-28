use crate::block::{Block, BlockKind};
use crate::text::{Line, is_blank, line_at};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Blank,
    Frontmatter,
    Heading,
    Paragraph,
    BlockQuote,
    Callout,
    ColonCallout,
    ListItem,
    Code,
    Table,
    ThematicBreak,
    Math,
    Html,
    Comment,
    Footnote,
    LinkReference,
}

#[must_use]
pub(crate) const fn kind_of(kind: &BlockKind) -> LineKind {
    match kind {
        BlockKind::Frontmatter(_) => LineKind::Frontmatter,
        BlockKind::Heading { .. } => LineKind::Heading,
        BlockKind::Paragraph => LineKind::Paragraph,
        BlockKind::BlockQuote => LineKind::BlockQuote,
        BlockKind::Callout { .. } => LineKind::Callout,
        BlockKind::ColonCallout { .. } => LineKind::ColonCallout,
        BlockKind::BulletList | BlockKind::OrderedList { .. } | BlockKind::ListItem { .. } => {
            LineKind::ListItem
        }
        BlockKind::CodeBlock { .. } => LineKind::Code,
        BlockKind::ThematicBreak => LineKind::ThematicBreak,
        BlockKind::MathBlock { .. } => LineKind::Math,
        BlockKind::Table(_) => LineKind::Table,
        BlockKind::HtmlBlock => LineKind::Html,
        BlockKind::Comment => LineKind::Comment,
        BlockKind::FootnoteDefinition { .. } => LineKind::Footnote,
        BlockKind::LinkReference { .. } => LineKind::LinkReference,
    }
}

pub(crate) fn build(source: &str, lines: &[Line], blocks: &[Block]) -> Vec<LineKind> {
    let mut kinds: Vec<LineKind> = lines
        .iter()
        .map(|line| {
            if is_blank(line.text(source)) {
                LineKind::Blank
            } else {
                LineKind::Paragraph
            }
        })
        .collect();
    for block in blocks {
        assign(lines, block, &mut kinds);
    }
    kinds
}

fn assign(lines: &[Line], block: &Block, kinds: &mut [LineKind]) {
    let kind = kind_of(&block.kind);
    let first = line_at(lines, block.span.start);
    let last = line_at(
        lines,
        block.span.end.saturating_sub(1).max(block.span.start),
    );
    for index in first..=last {
        if let Some(entry) = kinds.get_mut(index) {
            let container = matches!(kind, LineKind::Paragraph)
                && !matches!(entry, LineKind::Blank | LineKind::Paragraph);
            if !container {
                *entry = kind;
            }
        }
    }
    for child in &block.children {
        assign(lines, child, kinds);
    }
}
