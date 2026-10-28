use gpui_kit::{Context, Pixels, Window};
use margin_editor_commands::SelectionMode;
use margin_table_editing::{Direction, enter_table, step_across, vertical_cell};
use margin_text_buffer::{
    EditOrigin, Selection, line_end, line_start, next_grapheme_boundary, next_word_boundary,
    prev_grapheme_boundary, prev_word_boundary,
};

use crate::editing;
use crate::hit_map::HitMap;
use crate::note_editor::{Edited, NoteEditor};
use crate::surfaces::{self, MenuKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
    Up,
    Down,
    PageUp,
    PageDown,
    ParagraphUp,
    ParagraphDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Extend {
    Move,
    Select,
}

impl Motion {
    const fn is_vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::PageUp | Self::PageDown)
    }
}

pub(crate) fn run(
    editor: &mut NoteEditor,
    motion: Motion,
    extend: Extend,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if !editor.is_loaded() {
        return;
    }
    if motion == Motion::LineStart {
        let mode = match extend {
            Extend::Move => SelectionMode::Move,
            Extend::Select => SelectionMode::Extend,
        };
        editor.pointer_mut().set_goal_x(None);
        editing::line_start_motion(editor, mode, window, cx);
        return;
    }
    if extend == Extend::Move && special_motion(editor, motion, window, cx) {
        return;
    }
    if motion == Motion::Up
        && extend == Extend::Move
        && editor.session().selections().len() == 1
        && editor
            .session()
            .document()
            .line_of(editor.session().primary().head)
            == 0
    {
        surfaces::focus_above_body(editor, window, cx);
        return;
    }
    let hits = editor.hits();
    let page = editor.session().list().viewport_bounds().size.height;
    let goal = editor.pointer_mut().goal_x();
    let mut next_goal: Option<Pixels> = None;
    let session = editor.session();
    let text = session.text();
    let primary = session.buffer().primary_index();
    let selections: Vec<Selection> = session
        .selections()
        .iter()
        .map(|selection| {
            if extend == Extend::Move && !selection.is_empty() {
                match motion {
                    Motion::Left | Motion::WordLeft => return Selection::cursor(selection.start()),
                    Motion::Right | Motion::WordRight => return Selection::cursor(selection.end()),
                    _ => {}
                }
            }
            let head = selection.head;
            let target = match motion {
                Motion::Left => prev_grapheme_boundary(text, head),
                Motion::Right => next_grapheme_boundary(text, head),
                Motion::WordLeft => prev_word_boundary(text, head),
                Motion::WordRight => next_word_boundary(text, head),
                Motion::LineStart => line_start(text, head),
                Motion::LineEnd => line_end(text, head),
                Motion::DocStart => 0,
                Motion::DocEnd => text.len(),
                Motion::ParagraphUp => paragraph_start(text, head),
                Motion::ParagraphDown => paragraph_end(text, head),
                Motion::Up | Motion::Down | Motion::PageUp | Motion::PageDown => {
                    let caret = hits.caret_bounds(head);
                    let x = goal.or_else(|| caret.map(|bounds| bounds.left()));
                    if next_goal.is_none() {
                        next_goal = x;
                    }
                    vertical(text, head, motion, x, page, &hits)
                }
            };
            match extend {
                Extend::Move => Selection::cursor(target),
                Extend::Select => Selection::new(selection.anchor, target),
            }
        })
        .collect();
    editor.session_mut().set_selections(selections, primary);
    editor.pointer_mut().set_goal_x(if motion.is_vertical() {
        next_goal
    } else {
        None
    });
    editor.edited(Edited::Selection, window, cx);
}

fn special_motion(
    editor: &mut NoteEditor,
    motion: Motion,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) -> bool {
    let menu = match motion {
        Motion::Up => Some(MenuKey::Up),
        Motion::Down => Some(MenuKey::Down),
        _ => None,
    };
    if let Some(key) = menu
        && surfaces::menu_key(editor, key, window, cx)
    {
        return true;
    }
    if editor.session().selections().len() != 1 {
        return false;
    }
    editing::apply_change(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, _| match motion {
            Motion::Up => vertical_cell(document, selections, Direction::Backward)
                .or_else(|| enter_table(document, selections, Direction::Backward)),
            Motion::Down => vertical_cell(document, selections, Direction::Forward)
                .or_else(|| enter_table(document, selections, Direction::Forward)),
            Motion::Left => step_across(document, selections, Direction::Backward),
            Motion::Right => step_across(document, selections, Direction::Forward),
            _ => None,
        },
    )
}

fn vertical(
    text: &str,
    head: usize,
    motion: Motion,
    x: Option<Pixels>,
    page: Pixels,
    hits: &HitMap,
) -> usize {
    let below = matches!(motion, Motion::Down | Motion::PageDown);
    if let (Some(x), Motion::Up | Motion::Down) = (x, motion)
        && let Some(offset) = hits.vertical_neighbor(head, x, below)
        && offset != head
    {
        return offset;
    }
    if let (Some(x), Motion::PageUp | Motion::PageDown) = (x, motion)
        && let Some(caret) = hits.caret_bounds(head)
    {
        let y = if below {
            caret.top() + page
        } else {
            caret.top() - page
        };
        if let Some(offset) = hits.offset_at(gpui_kit::point(x, y)) {
            return offset;
        }
    }
    logical_vertical(text, head, below)
}

fn logical_vertical(text: &str, head: usize, below: bool) -> usize {
    let start = line_start(text, head);
    let column = text
        .get(start..head)
        .map_or(0, |prefix| prefix.chars().count());
    let target_start = if below {
        let end = line_end(text, head);
        if end >= text.len() {
            return text.len();
        }
        end + 1
    } else {
        if start == 0 {
            return 0;
        }
        line_start(text, start - 1)
    };
    let target_end = line_end(text, target_start);
    let line = text.get(target_start..target_end).unwrap_or_default();
    target_start
        + line
            .char_indices()
            .nth(column)
            .map_or(line.len(), |(index, _)| index)
}

fn paragraph_start(text: &str, head: usize) -> usize {
    let start = line_start(text, head);
    if head > start || start == 0 {
        start
    } else {
        line_start(text, start - 1)
    }
}

fn paragraph_end(text: &str, head: usize) -> usize {
    let end = line_end(text, head);
    if head < end || end >= text.len() {
        end
    } else {
        line_end(text, end + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_vertical_keeps_the_column_and_clamps() {
        let text = "abcdef\nab\nabcdef";
        assert_eq!(logical_vertical(text, 4, true), 9);
        assert_eq!(logical_vertical(text, 9, true), 12);
        assert_eq!(logical_vertical(text, 12, false), 9);
        assert_eq!(logical_vertical(text, 2, false), 0);
        assert_eq!(logical_vertical(text, 12, true), text.len());
    }

    #[test]
    fn paragraph_motions_stop_at_line_edges_then_cross() {
        let text = "one\ntwo\nthree";
        assert_eq!(paragraph_start(text, 6), 4);
        assert_eq!(paragraph_start(text, 4), 0);
        assert_eq!(paragraph_start(text, 0), 0);
        assert_eq!(paragraph_end(text, 5), 7);
        assert_eq!(paragraph_end(text, 7), text.len());
        assert_eq!(paragraph_end(text, text.len()), text.len());
    }
}
