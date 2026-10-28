use gpui_kit::{Context, Window, point, px};
use margin_editor_commands::{
    Direction, Granularity, SelectionMode, autopair_input, delete_backward, delete_forward,
    delete_line as delete_line_change, delete_pair_backward, delete_to_line_start,
    indent as indent_change, insert_newline, insert_text, line_content_start, move_list_item,
    outdent as outdent_change, soft_break as soft_break_change,
};
use margin_markdown::Document;
use margin_table_editing::{
    PipeTyped, cell_at, enter_cell, filter_cell_input, insert_break, keep_table_gaps,
    leave_delimiter, next_cell, previous_cell, select_table_above, type_pipe,
};
use margin_text_buffer::{Change, EditOrigin, Selection, line_start};

use crate::hit_map::HitMap;
use crate::movement::{self, Extend, Motion};
use crate::note_editor::{Edited, NoteEditor};
use crate::surfaces::{self, MenuKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Deletion {
    Backward,
    Forward,
    WordBackward,
    WordForward,
    LineStart,
    LineEnd,
}

fn run(
    editor: &mut NoteEditor,
    origin: EditOrigin,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
    command: impl FnOnce(&Document, &[Selection], usize) -> Option<Change>,
) -> bool {
    if !editor.is_loaded() {
        return false;
    }
    let session = editor.session();
    let primary = session.buffer().primary_index();
    let Some(change) = command(session.document(), session.selections(), primary) else {
        return false;
    };
    editor.apply(change, origin, window, cx);
    true
}

fn in_table(editor: &NoteEditor) -> bool {
    let session = editor.session();
    session.selections().len() == 1 && cell_at(session.document(), session.primary().head).is_some()
}

pub(crate) fn type_text(
    editor: &mut NoteEditor,
    text: &str,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if !editor.is_loaded() {
        return;
    }
    if in_table(editor) {
        if text == "|" {
            let session = editor.session();
            match type_pipe(session.document(), session.selections()) {
                Some(PipeTyped::Moved(change)) => {
                    editor.apply(change, EditOrigin::Typing, window, cx);
                    return;
                }
                Some(PipeTyped::Swallowed) => return,
                Some(PipeTyped::ClosedRow) => {
                    run(
                        editor,
                        EditOrigin::Typing,
                        window,
                        cx,
                        |document, selections, _| enter_cell(document, selections, Some(0)),
                    );
                    return;
                }
                None => {}
            }
        }
        let filtered = filter_cell_input(text).into_owned();
        run(
            editor,
            EditOrigin::Typing,
            window,
            cx,
            |document, selections, _| Some(insert_text(document, selections, &filtered)),
        );
        return;
    }
    run(
        editor,
        EditOrigin::Typing,
        window,
        cx,
        |document, selections, primary| {
            let change = autopair_input(document, selections, primary, text)
                .unwrap_or_else(|| insert_text(document, selections, text));
            Some(keep_table_gaps(document, selections, change))
        },
    );
}

pub(crate) fn delete(
    editor: &mut NoteEditor,
    deletion: Deletion,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let hits = editor.hits();
    let text = editor.session().text().to_owned();
    run(
        editor,
        EditOrigin::Deletion,
        window,
        cx,
        |document, selections, _| match deletion {
            Deletion::Backward => select_table_above(document, selections)
                .map(|(change, _)| change)
                .or_else(|| delete_pair_backward(document, selections))
                .or_else(|| Some(delete_backward(document, selections, Granularity::Grapheme))),
            Deletion::Forward => Some(delete_forward(document, selections, Granularity::Grapheme)),
            Deletion::WordBackward => {
                Some(delete_backward(document, selections, Granularity::Word))
            }
            Deletion::WordForward => Some(delete_forward(document, selections, Granularity::Word)),
            Deletion::LineStart => delete_to_line_start(document, selections, |offset| {
                visual_row_start(&hits, &text, offset)
            }),
            Deletion::LineEnd => Some(delete_forward(
                document,
                selections,
                Granularity::LineBoundary,
            )),
        },
    );
}

pub(crate) fn visual_row_start(hits: &HitMap, text: &str, offset: usize) -> usize {
    hits.caret_bounds(offset)
        .and_then(|caret| hits.offset_at(point(px(-1.0e6), caret.center().y)))
        .filter(|start| *start <= offset && *start >= line_start(text, offset))
        .unwrap_or_else(|| line_start(text, offset))
}

pub(crate) fn line_start_motion(
    editor: &mut NoteEditor,
    mode: SelectionMode,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let hits = editor.hits();
    let text = editor.session().text().to_owned();
    run(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, _| {
            line_content_start(document, selections, mode, |offset| {
                visual_row_start(&hits, &text, offset)
            })
        },
    );
}

pub(crate) fn newline(editor: &mut NoteEditor, window: &Window, cx: &mut Context<'_, NoteEditor>) {
    if surfaces::menu_key(editor, MenuKey::Confirm, window, cx) {
        return;
    }
    if in_table(editor)
        && run(
            editor,
            EditOrigin::Typing,
            window,
            cx,
            |document, selections, _| {
                enter_cell(document, selections, None)
                    .or_else(|| leave_delimiter(document, selections))
            },
        )
    {
        return;
    }
    run(
        editor,
        EditOrigin::Typing,
        window,
        cx,
        |document, selections, primary| Some(insert_newline(document, selections, primary)),
    );
}

pub(crate) fn soft_break(
    editor: &mut NoteEditor,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    run(
        editor,
        EditOrigin::Typing,
        window,
        cx,
        |document, selections, _| {
            insert_break(document, selections)
                .or_else(|| soft_break_change(document, selections))
                .or_else(|| Some(insert_text(document, selections, "\n")))
        },
    );
}

pub(crate) fn indent(editor: &mut NoteEditor, window: &Window, cx: &mut Context<'_, NoteEditor>) {
    if surfaces::menu_key(editor, MenuKey::Confirm, window, cx) {
        return;
    }
    run(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, _| {
            next_cell(document, selections).or_else(|| Some(indent_change(document, selections)))
        },
    );
}

pub(crate) fn outdent(editor: &mut NoteEditor, window: &Window, cx: &mut Context<'_, NoteEditor>) {
    run(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, _| {
            previous_cell(document, selections)
                .or_else(|| Some(outdent_change(document, selections)))
        },
    );
}

pub(crate) fn undo(editor: &mut NoteEditor, window: &Window, cx: &mut Context<'_, NoteEditor>) {
    if editor.is_loaded() && editor.session_mut().undo().is_some() {
        editor.edited(Edited::Text, window, cx);
    }
}

pub(crate) fn redo(editor: &mut NoteEditor, window: &Window, cx: &mut Context<'_, NoteEditor>) {
    if editor.is_loaded() && editor.session_mut().redo().is_some() {
        editor.edited(Edited::Text, window, cx);
    }
}

pub(crate) fn select_all(
    editor: &mut NoteEditor,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let end = editor.session().text().len();
    editor.session_mut().select(Selection::new(0, end));
    editor.edited(Edited::Selection, window, cx);
}

pub(crate) fn delete_line(
    editor: &mut NoteEditor,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    run(
        editor,
        EditOrigin::Deletion,
        window,
        cx,
        |document, selections, _| Some(delete_line_change(document, selections)),
    );
}

pub(crate) fn move_line(
    editor: &mut NoteEditor,
    down: bool,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let direction = if down { Direction::Down } else { Direction::Up };
    let moved = run(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, _| move_list_item(document, selections, direction),
    );
    if !moved {
        let motion = if down {
            Motion::ParagraphDown
        } else {
            Motion::ParagraphUp
        };
        movement::run(editor, motion, Extend::Move, window, cx);
    }
}

pub(crate) fn apply_change(
    editor: &mut NoteEditor,
    origin: EditOrigin,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
    command: impl FnOnce(&Document, &[Selection], usize) -> Option<Change>,
) -> bool {
    run(editor, origin, window, cx, command)
}
