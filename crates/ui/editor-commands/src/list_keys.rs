use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::list_move::{self, Target};
use crate::list_tree::{self, ListBlock};
use crate::mapping::{self, Assoc};
use crate::text::{self, Line};
use crate::{indentation, tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub(crate) const fn step(self) -> isize {
        match self {
            Self::Up => -1,
            Self::Down => 1,
        }
    }
}

struct Picked {
    block: ListBlock,
    items: Vec<usize>,
}

fn picked(document: &Document, selections: &[Selection]) -> Option<Picked> {
    let [range] = selections else {
        return None;
    };
    let first = Line::at(document, range.start());
    let last = Line::at(document, range.end());
    let block = list_tree::list_at(
        document,
        first.start + text::leading_space(first.text).len(),
    )?;
    let mut items: Vec<usize> = (0..block.items.len())
        .filter(|index| {
            let from = block.item(*index).from;
            from >= first.start && from <= last.end
        })
        .collect();
    if items.is_empty() {
        items.extend(block.item_at(range.head));
    }
    let contains = |outer: usize, inner: usize| {
        let (outer, inner) = (block.item(outer), block.item(inner));
        outer != inner && outer.from <= inner.from && inner.to <= outer.to
    };
    let kept: Vec<usize> = items
        .iter()
        .copied()
        .filter(|item| !items.iter().any(|other| contains(*other, *item)))
        .collect();
    (!kept.is_empty()).then_some(Picked { block, items: kept })
}

fn shift(
    document: &Document,
    selections: &[Selection],
    picked: &Picked,
    columns: usize,
    style: list_tree::Indent,
    token: Option<&str>,
) -> Change {
    let block = &picked.block;
    let indent_columns =
        |index: usize| text::columns(list_tree::indent_of(document, block.item(index)));
    let Some(head) = picked.items.first().copied() else {
        return Change::default();
    };
    let delta = mapping::signed(columns) - mapping::signed(indent_columns(head));
    let mut edits = Vec::new();
    let mut maps = Vec::new();
    for (position, index) in picked.items.iter().enumerate() {
        let item = block.item(*index);
        let target =
            usize::try_from((mapping::signed(indent_columns(*index)) + delta).max(0)).unwrap_or(0);
        let token = if position == 0 { token } else { None };
        let releveled = list_tree::relevel(document, item, target, style, token);
        edits.push(TextEdit::new(
            Span::new(item.from, item.to),
            releveled.text.clone(),
        ));
        maps.push((item, releveled));
    }
    let head_from = block.item(head).from;
    list_tree::list_edit(
        document,
        edits,
        |edits| vec![mapping::map_pos(edits, head_from, Assoc::Before)],
        |edits| {
            let move_to = |pos: usize| match maps
                .iter()
                .find(|(item, _)| item.from <= pos && pos <= item.to)
            {
                Some((item, releveled)) => {
                    mapping::map_pos(edits, item.from, Assoc::Before)
                        + releveled.map(pos - item.from)
                }
                None => mapping::map_pos(edits, pos, Assoc::After),
            };
            selections
                .iter()
                .map(|selection| Selection::new(move_to(selection.anchor), move_to(selection.head)))
                .collect()
        },
    )
    .unwrap_or_default()
}

fn indent_items(document: &Document, selections: &[Selection]) -> Option<Change> {
    let found = picked(document, selections)?;
    let block = &found.block;
    let head = *found.items.first()?;
    let Some(previous) = block.sibling(head, -1) else {
        return Some(Change::default());
    };
    let previous = block.item(previous);
    let style = list_tree::indent_style(
        document,
        &[
            list_tree::indent_of(document, previous),
            list_tree::indent_of(document, block.item(head)),
        ],
    );
    let last = previous.children.last().map(|child| block.item(*child));
    let columns = match last {
        Some(last) => text::columns(list_tree::indent_of(document, last)),
        None => list_tree::child_column(document, previous, style),
    };
    let token = if found.items.len() == 1 {
        list_tree::token_for(document, last, block.item(head))
    } else {
        None
    };
    Some(shift(
        document,
        selections,
        &found,
        columns,
        style,
        token.as_deref(),
    ))
}

fn outdent_items(document: &Document, selections: &[Selection]) -> Option<Change> {
    let found = picked(document, selections)?;
    let block = &found.block;
    let head = *found.items.first()?;
    let Some(parent) = block.item(head).parent.map(|parent| block.item(parent)) else {
        return Some(Change::default());
    };
    let parent_indent = list_tree::indent_of(document, parent);
    let style = list_tree::indent_style(
        document,
        &[
            parent_indent,
            list_tree::indent_of(document, block.item(head)),
        ],
    );
    let token = if found.items.len() == 1 {
        list_tree::token_for(document, Some(parent), block.item(head))
    } else {
        None
    };
    Some(shift(
        document,
        selections,
        &found,
        text::columns(parent_indent),
        style,
        token.as_deref(),
    ))
}

fn in_code(document: &Document, selections: &[Selection]) -> bool {
    selections
        .first()
        .is_some_and(|selection| tree::in_code_block(document, selection.head))
}

#[must_use]
pub fn indent(document: &Document, selections: &[Selection]) -> Change {
    (!in_code(document, selections))
        .then(|| indent_items(document, selections))
        .flatten()
        .unwrap_or_else(|| indentation::indent_more(document, selections))
}

#[must_use]
pub fn outdent(document: &Document, selections: &[Selection]) -> Change {
    (!in_code(document, selections))
        .then(|| outdent_items(document, selections))
        .flatten()
        .unwrap_or_else(|| indentation::indent_less(document, selections))
}

#[must_use]
pub fn move_list_item(
    document: &Document,
    selections: &[Selection],
    direction: Direction,
) -> Option<Change> {
    let found = picked(document, selections)?;
    let [index] = found.items.as_slice() else {
        return None;
    };
    let block = &found.block;
    let item = block.item(*index);
    let own_indent = list_tree::indent_of(document, item);
    let style = list_tree::indent_style(document, &[own_indent]);
    let step = direction.step();
    if let Some(neighbor) = block
        .sibling(*index, step)
        .map(|neighbor| block.item(neighbor))
    {
        let target = match direction {
            Direction::Up => Target {
                pos: neighbor.from,
                lead: "",
                tail: "\n",
            },
            Direction::Down => Target {
                pos: neighbor.to,
                lead: "\n",
                tail: "",
            },
        };
        let columns = text::columns(own_indent);
        return Some(
            list_move::relocate_item(document, selections, item, &target, columns, style, None)
                .unwrap_or_default(),
        );
    }
    let Some(uncle) = item.parent.and_then(|parent| block.sibling(parent, step)) else {
        return Some(Change::default());
    };
    let uncle = block.item(uncle);
    let edge = match direction {
        Direction::Up => uncle.children.last(),
        Direction::Down => uncle.children.first(),
    }
    .map(|edge| block.item(*edge));
    let columns = match edge {
        Some(edge) => text::columns(list_tree::indent_of(document, edge)),
        None => list_tree::child_column(document, uncle, style),
    };
    let target = match (direction, edge) {
        (Direction::Down, Some(edge)) => Target {
            pos: edge.from,
            lead: "",
            tail: "\n",
        },
        _ => Target {
            pos: uncle.to,
            lead: "\n",
            tail: "",
        },
    };
    let token = list_tree::token_for(document, edge, item);
    Some(
        list_move::relocate_item(
            document,
            selections,
            item,
            &target,
            columns,
            style,
            token.as_deref(),
        )
        .unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::run;

    fn tab(source: &str) -> String {
        run(source, |d, s| Some(indent(d, s))).expect("indent always applies")
    }

    fn shift_tab(source: &str) -> String {
        run(source, |d, s| Some(outdent(d, s))).expect("outdent always applies")
    }

    #[test]
    fn indents_an_item_under_its_previous_sibling() {
        assert_eq!(tab("- a\n- b|\n"), "- a\n\t- b|\n");
        assert_eq!(tab("1. a\n2. b|\n3. c\n"), "1. a\n\t1. b|\n2. c\n");
        assert_eq!(tab("- a\n  - x\n- b|\n"), "- a\n  - x\n  - b|\n");
        assert_eq!(tab("- a|\n- b\n"), "- a|\n- b\n");
    }

    #[test]
    fn indents_with_spaces_when_the_note_indents_items_with_spaces() {
        assert_eq!(tab("- a\n  - x\n- b\n- c|\n"), "- a\n  - x\n- b\n  - c|\n");
    }

    #[test]
    fn moves_a_subtree_with_its_item() {
        assert_eq!(tab("- a\n- b|\n  - c\n"), "- a\n  - b|\n    - c\n");
        assert_eq!(shift_tab("- a\n  - b|\n    - c\n"), "- a\n- b|\n  - c\n");
    }

    #[test]
    fn outdents_and_renumbers() {
        assert_eq!(shift_tab("1. a\n   1. b|\n2. c\n"), "1. a\n2. b|\n3. c\n");
        assert_eq!(shift_tab("- a|\n"), "- a|\n");
    }

    #[test]
    fn falls_back_to_plain_indentation_outside_lists() {
        assert_eq!(tab("te|xt"), "\tte|xt");
        assert_eq!(shift_tab("\tte|xt"), "te|xt");
    }

    #[test]
    fn moves_items_among_siblings_and_across_parents() {
        let up = |source| run(source, |d, s| move_list_item(d, s, Direction::Up));
        let down = |source| run(source, |d, s| move_list_item(d, s, Direction::Down));
        assert_eq!(up("- a\n- b|\n").as_deref(), Some("- b|\n- a\n"));
        assert_eq!(
            down("1. a|\n2. b\n3. c\n").as_deref(),
            Some("1. b\n2. a|\n3. c\n")
        );
        assert_eq!(
            down("- a\n  - x|\n- b\n  - y\n").as_deref(),
            Some("- a\n- b\n  - x|\n  - y\n")
        );
        assert_eq!(up("- a|\n").as_deref(), Some("- a|\n"));
        assert_eq!(up("plain|"), None);
    }
}
