use gpui_kit::{
    Bounds, Context, Div, DragMoveEvent, ExternalPaths, InteractiveElement, Pixels, Point,
    Stateful, Styled, Window,
};
use margin_text_buffer::Selection;
use margin_theme::MarginTheme;
use margin_workbench::DraggedFiles;

use crate::clipboard;
use crate::note_editor::{Edited, NoteEditor};

pub(crate) fn wire(body: Stateful<Div>, cx: &Context<'_, NoteEditor>) -> Stateful<Div> {
    let palette = MarginTheme::palette(cx);
    let (fill, border) = (palette.brand(8.0), palette.border_strong);
    body.on_drag_move(
        cx.listener(|editor, event: &DragMoveEvent<ExternalPaths>, window, cx| {
            drag_hover(editor, event.event.position, event.bounds, window, cx);
        }),
    )
    .on_drag_move(
        cx.listener(|editor, event: &DragMoveEvent<DraggedFiles>, window, cx| {
            if !event.drag(cx).has_folder {
                drag_hover(editor, event.event.position, event.bounds, window, cx);
            }
        }),
    )
    .drag_over::<ExternalPaths>(move |style, _, _, _| {
        style.bg(fill).border_1().border_color(border)
    })
    .drag_over::<DraggedFiles>(move |style, dragged, _, _| {
        if dragged.has_folder {
            style
        } else {
            style.bg(fill).border_1().border_color(border)
        }
    })
    .on_drop(cx.listener(|editor, paths: &ExternalPaths, window, cx| {
        drop_files(editor, paths, window, cx);
    }))
    .on_drop(cx.listener(|editor, dragged: &DraggedFiles, window, cx| {
        drop_vault_files(editor, dragged, window, cx);
    }))
}

fn drag_hover(
    editor: &mut NoteEditor,
    position: Point<Pixels>,
    bounds: Bounds<Pixels>,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if !editor.is_loaded() || !bounds.contains(&position) {
        return;
    }
    let Some(offset) = editor.hits().offset_at(position) else {
        return;
    };
    let caret = Selection::cursor(offset);
    if editor.session().selections() != [caret] {
        editor.session_mut().select(caret);
        editor.edited(Edited::Selection, window, cx);
    }
}

fn place_at_drop(
    editor: &mut NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) -> bool {
    if !editor.is_loaded() {
        return false;
    }
    if let Some(offset) = editor.hits().offset_at(window.mouse_position()) {
        editor.session_mut().select(Selection::cursor(offset));
    }
    let focus = editor.focus_handle_ref().clone();
    window.focus(&focus, cx);
    true
}

fn drop_files(
    editor: &mut NoteEditor,
    paths: &ExternalPaths,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if place_at_drop(editor, window, cx) {
        clipboard::drop_paths(editor, paths.paths().to_vec(), cx);
    }
}

fn drop_vault_files(
    editor: &mut NoteEditor,
    dragged: &DraggedFiles,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if dragged.has_folder || !place_at_drop(editor, window, cx) {
        return;
    }
    let root = editor.services().root.clone();
    let paths = dragged
        .paths
        .iter()
        .map(|path| {
            path.components()
                .fold(root.clone(), |at, part| at.join(part))
        })
        .collect();
    clipboard::drop_paths(editor, paths, cx);
}
