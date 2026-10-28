use margin_markdown::Document;
use margin_text_buffer::{Change, Selection};

use crate::block_drop::{DragBlock, DragKind, Side};
use crate::list_move::{self, Target};
use crate::list_tree::{self, ListBlock};
use crate::text::{self, Line};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GapItem {
    pub mark: usize,
    pub level: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropGap {
    pub pos: usize,
    pub side: Side,
    pub min_level: usize,
    pub max_level: usize,
    pub next: Option<GapItem>,
    pub previous: Vec<GapItem>,
}

impl DropGap {
    fn reference(&self, level: usize) -> Option<GapItem> {
        if let Some(next) = self.next.filter(|next| next.level == level) {
            return Some(next);
        }
        let previous = self.previous.first()?;
        (level <= previous.level).then(|| {
            self.previous
                .iter()
                .copied()
                .find(|item| item.level <= level)
                .unwrap_or(*previous)
        })
    }
}

fn chain(list: &ListBlock, index: usize) -> Vec<GapItem> {
    let mut out = Vec::new();
    let mut at = Some(index);
    while let Some(current) = at {
        let item = list.item(current);
        out.push(GapItem {
            mark: item.mark,
            level: item.level,
        });
        at = item.parent;
    }
    out
}

#[must_use]
pub fn list_gaps(document: &Document, pos: usize, source: &DragBlock) -> Vec<DropGap> {
    let line = Line::at(document, pos);
    let Some(list) =
        list_tree::list_at(document, line.start + text::leading_space(line.text).len())
    else {
        return Vec::new();
    };
    let skip = (source.kind == DragKind::ListItem).then_some(source.span);
    let kept: Vec<usize> = (0..list.items.len())
        .filter(|index| {
            let item = list.item(*index);
            skip.is_none_or(|skip| item.from < skip.start || item.to > skip.end)
        })
        .collect();
    let mut gaps: Vec<DropGap> = Vec::with_capacity(kept.len() + 1);
    let mut previous: Option<usize> = None;
    for index in &kept {
        let item = list.item(*index);
        gaps.push(DropGap {
            pos: item.from,
            side: Side::Before,
            min_level: item.level,
            max_level: previous.map_or(item.level, |previous| list.item(previous).level + 1),
            next: Some(GapItem {
                mark: item.mark,
                level: item.level,
            }),
            previous: previous
                .map(|previous| chain(&list, previous))
                .unwrap_or_default(),
        });
        previous = Some(*index);
    }
    if let Some(last) = previous {
        let item = list.item(last);
        let holds_skip = skip.is_some_and(|skip| item.from < skip.start && skip.end <= item.to);
        let end = match skip {
            Some(skip) if holds_skip => Line::at(document, skip.start).start.saturating_sub(1),
            _ => item.to,
        };
        gaps.push(DropGap {
            pos: end,
            side: Side::After,
            min_level: 1,
            max_level: item.level + 1,
            next: None,
            previous: chain(&list, last),
        });
    }
    gaps
}

pub(crate) fn resolve(document: &Document, mark: usize) -> Option<(ListBlock, usize)> {
    let list = list_tree::list_at(document, mark)?;
    let index = list.index_of_mark(mark)?;
    Some((list, index))
}

#[must_use]
pub fn item_level(document: &Document, block: &DragBlock) -> Option<usize> {
    if block.kind != DragKind::ListItem {
        return None;
    }
    resolve(document, block.anchor).map(|(list, index)| list.item(index).level)
}

#[must_use]
pub fn drop_item(
    document: &Document,
    selections: &[Selection],
    source: &DragBlock,
    gap: &DropGap,
    level: usize,
) -> Option<Change> {
    if source.kind != DragKind::ListItem {
        return None;
    }
    let (source_list, source_index) = resolve(document, source.anchor)?;
    let item = source_list.item(source_index);
    let previous = gap
        .previous
        .first()
        .and_then(|entry| resolve(document, entry.mark));
    let next = gap.next.and_then(|entry| resolve(document, entry.mark));
    let indent_of = |found: &Option<(ListBlock, usize)>| {
        found.as_ref().map_or("", |(list, index)| {
            list_tree::indent_of(document, list.item(*index))
        })
    };
    let style = list_tree::indent_style(
        document,
        &[
            list_tree::indent_of(document, item),
            indent_of(&previous),
            indent_of(&next),
        ],
    );
    let reference = gap
        .reference(level)
        .and_then(|entry| resolve(document, entry.mark));
    let columns = match (&reference, &previous) {
        (Some((list, index)), _) => {
            text::columns(list_tree::indent_of(document, list.item(*index)))
        }
        (None, Some((list, index))) => list_tree::child_column(document, list.item(*index), style),
        (None, None) => 0,
    };
    let target = match gap.side {
        Side::After => Target {
            pos: gap.pos,
            lead: "\n",
            tail: "",
        },
        Side::Before => Target {
            pos: gap.pos,
            lead: "",
            tail: "\n",
        },
    };
    let token = list_tree::token_for(
        document,
        reference.as_ref().map(|(list, index)| list.item(*index)),
        item,
    );
    list_move::relocate_item(
        document,
        selections,
        item,
        &target,
        columns,
        style,
        token.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_drop::draggable_block;
    use crate::testing::apply;

    #[test]
    fn drops_a_list_item_into_a_gap_at_a_level() {
        let document = Document::parse("- a\n- b\n- c\n");
        let source = draggable_block(&document, 9).expect("item c");
        assert_eq!(item_level(&document, &source), Some(1));
        let gaps = list_gaps(&document, 0, &source);
        assert_eq!(gaps.len(), 3);
        let gap = gaps.get(1).expect("the gap before b");
        assert_eq!((gap.min_level, gap.max_level), (1, 2));
        let change = drop_item(&document, &[], &source, gap, 2).expect("dropping nests c under a");
        assert_eq!(apply(&document, &change), "- a\n\t- c\n- b\n");
    }
}
