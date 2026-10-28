use gpui_kit::{Context, Window};
use margin_editor_commands::{
    ListKind, MarkKind, insert_link as insert_link_change, toggle_checklist,
    toggle_heading as toggle_heading_change, toggle_list as toggle_list_change,
    toggle_mark as toggle_mark_change, toggle_quote as toggle_quote_change,
};
use margin_text_buffer::EditOrigin;

use crate::editing::apply_change;
use crate::note_editor::NoteEditor;

pub(crate) fn toggle_mark(
    editor: &mut NoteEditor,
    mark: MarkKind,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    apply_change(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, _| Some(toggle_mark_change(document, selections, mark)),
    );
}

pub(crate) fn insert_link(
    editor: &mut NoteEditor,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    apply_change(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, _| Some(insert_link_change(document, selections)),
    );
}

pub(crate) fn toggle_quote(
    editor: &mut NoteEditor,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    apply_change(editor, EditOrigin::Command, window, cx, toggle_quote_change);
}

pub(crate) fn toggle_list(
    editor: &mut NoteEditor,
    kind: ListKind,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    apply_change(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, primary| toggle_list_change(document, selections, primary, kind),
    );
}

pub(crate) fn toggle_heading(
    editor: &mut NoteEditor,
    level: u8,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    apply_change(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, primary| toggle_heading_change(document, selections, primary, level),
    );
}

pub(crate) fn cycle_checklist(
    editor: &mut NoteEditor,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    apply_change(
        editor,
        EditOrigin::Command,
        window,
        cx,
        |document, selections, _| Some(toggle_checklist(document, selections)),
    );
}
