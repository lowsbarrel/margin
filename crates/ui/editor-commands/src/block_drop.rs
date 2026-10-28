use margin_markdown::{BlockKind, Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::list_move::{self, Target};
use crate::mapping::{self, Assoc};
use crate::text::{self, Line};
use crate::{list_drop, list_tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragKind {
    ListItem,
    Table,
    Frontmatter,
    Block,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DragBlock {
    pub span: Span,
    pub anchor: usize,
    pub kind: DragKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Before,
    After,
}

fn block_at(document: &Document, pos: usize, items: bool) -> Option<DragBlock> {
    let line = Line::at(document, pos);
    if line.is_blank() {
        return None;
    }
    let start = line.start + text::leading_space(line.text).len();
    if items
        && let Some(list) = list_tree::list_at(document, start)
        && let Some(item) = list.item_at(start)
    {
        let item = list.item(item);
        return Some(DragBlock {
            span: Span::new(item.from, item.to),
            anchor: item.mark,
            kind: DragKind::ListItem,
        });
    }
    let top = document.blocks().iter().find(|block| {
        block.span.start <= start && start < block.span.end.max(block.span.start + 1)
    })?;
    let span = list_tree::line_span(document, top.span.start, top.span.end);
    let kind = match top.kind {
        BlockKind::Table(_) => DragKind::Table,
        BlockKind::Frontmatter(_) => DragKind::Frontmatter,
        _ => DragKind::Block,
    };
    Some(DragBlock {
        span,
        anchor: span.start,
        kind,
    })
}

#[must_use]
pub fn draggable_block(document: &Document, pos: usize) -> Option<DragBlock> {
    block_at(document, pos, true).filter(|block| block.kind != DragKind::Frontmatter)
}

#[must_use]
pub fn block_near(document: &Document, pos: usize) -> Option<(DragBlock, Option<Side>)> {
    let line = Line::at(document, pos);
    if !line.is_blank() {
        return block_at(document, pos, false).map(|block| (block, None));
    }
    let filled =
        |index: usize| Some(Line::numbered(document, index)).filter(|line| !line.is_blank());
    if let Some(above) = (0..line.index).rev().find_map(filled) {
        return block_at(document, above.start, false).map(|block| (block, Some(Side::After)));
    }
    let below = (line.index + 1..document.line_count()).find_map(filled)?;
    block_at(document, below.start, false).map(|block| (block, Some(Side::Before)))
}

fn beside(document: &Document, block: &DragBlock, side: Side) -> Target {
    match side {
        Side::After => {
            let last = Line::at(document, block.span.end);
            let next_filled = last.index + 1 < document.line_count()
                && !Line::numbered(document, last.index + 1).is_blank();
            Target {
                pos: block.span.end,
                lead: "\n\n",
                tail: if next_filled { "\n" } else { "" },
            }
        }
        Side::Before => {
            let first = Line::at(document, block.span.start);
            let previous_filled = first
                .index
                .checked_sub(1)
                .is_some_and(|index| !Line::numbered(document, index).is_blank());
            Target {
                pos: block.span.start,
                lead: if previous_filled { "\n" } else { "" },
                tail: "\n\n",
            }
        }
    }
}

fn block_removal(document: &Document, span: Span) -> Span {
    let first = Line::at(document, span.start);
    let last = Line::at(document, span.end);
    let filled =
        |index: usize| Some(Line::numbered(document, index)).filter(|line| !line.is_blank());
    if let Some(after) = (last.index + 1..document.line_count()).find_map(filled) {
        return Span::new(span.start, after.start);
    }
    if let Some(before) = (0..first.index).rev().find_map(filled) {
        return Span::new(before.end, span.end);
    }
    span
}

#[must_use]
pub fn drop_beside(
    document: &Document,
    selections: &[Selection],
    dragged: &DragBlock,
    target: &DragBlock,
    side: Side,
) -> Option<Change> {
    if target.span.start <= dragged.span.end && dragged.span.start <= target.span.end {
        return None;
    }
    let spot = beside(document, target, side);
    if dragged.kind == DragKind::ListItem {
        let (list, index) = list_drop::resolve(document, dragged.anchor)?;
        let item = list.item(index);
        let style = list_tree::indent_style(document, &[list_tree::indent_of(document, item)]);
        let token = list_tree::token_for(document, None, item);
        return list_move::relocate_item(
            document,
            selections,
            item,
            &spot,
            0,
            style,
            token.as_deref(),
        );
    }
    let cut = block_removal(document, dragged.span);
    if spot.pos > cut.start && spot.pos < cut.end {
        return None;
    }
    let moved = dragged.span.slice(document.text());
    let edits = vec![
        TextEdit::new(cut, ""),
        TextEdit::new(
            Span::empty(spot.pos),
            format!("{}{moved}{}", spot.lead, spot.tail),
        ),
    ];
    list_tree::list_edit(
        document,
        edits,
        |_| Vec::new(),
        |edits| {
            let start = mapping::map_pos(edits, spot.pos, Assoc::Before) + spot.lead.len();
            let move_to = |pos: usize| {
                if dragged.span.start <= pos && pos <= dragged.span.end {
                    start + pos - dragged.span.start
                } else {
                    mapping::map_pos(edits, pos, Assoc::Before)
                }
            };
            selections
                .iter()
                .map(|selection| Selection::new(move_to(selection.anchor), move_to(selection.head)))
                .collect()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::apply;

    fn moved(source: &str, from: usize, to: usize, side: Side) -> Option<String> {
        let document = Document::parse(source);
        let dragged = draggable_block(&document, from)?;
        let (target, _) = block_near(&document, to)?;
        let change = drop_beside(&document, &[Selection::cursor(0)], &dragged, &target, side)?;
        Some(apply(&document, &change))
    }

    #[test]
    fn finds_the_dragged_block() {
        let document = Document::parse("---\na: 1\n---\n\n# Title\n\n- a\n  - b\n");
        assert_eq!(draggable_block(&document, 0), None);
        let heading = draggable_block(&document, 15).expect("the heading is draggable");
        assert_eq!(
            (heading.span, heading.kind),
            (Span::new(14, 21), DragKind::Block)
        );
        let item = draggable_block(&document, 27).expect("the nested item is draggable");
        assert_eq!(
            (item.span, item.kind),
            (Span::new(27, 32), DragKind::ListItem)
        );
    }

    #[test]
    fn moves_blocks_before_and_after_others() {
        assert_eq!(
            moved("one\n\ntwo\n\nthree", 0, 10, Side::After).as_deref(),
            Some("two\n\nthree\n\none")
        );
        assert_eq!(
            moved("one\n\ntwo\n\nthree", 10, 0, Side::Before).as_deref(),
            Some("three\n\none\n\ntwo")
        );
        assert_eq!(moved("one\n\ntwo", 0, 0, Side::After), None);
    }
}
