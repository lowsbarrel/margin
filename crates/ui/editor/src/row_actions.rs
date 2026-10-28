use gpui_kit::{App, ClipboardItem, Context, ImageSource, SharedString, Window};
use margin_editor_assets::{Embed, ImageLocation, VaultIndex, resolve_embed, resolve_image};
use margin_editor_overlays::LightboxImage;
use margin_kernel::RelativePath;
use margin_markdown::{Block, Inline, InlineKind, Span, TextEdit};
use margin_table_editing::{apply_table_op, cell_at};
use margin_text_buffer::{Change, EditOrigin, Selection};
use margin_workbench::{OpenRequest, WorkbenchEvent};

use crate::note_editor::{Edited, NoteEditor};
use crate::row_context::RowAction;
use crate::surfaces;

pub(crate) fn handle(
    editor: &mut NoteEditor,
    action: RowAction,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    match action {
        RowAction::ToggleTask { marker } => toggle_task(editor, marker, window, cx),
        RowAction::ToggleFold { block_start, fold } => {
            editor.session_mut().toggle_fold(block_start, fold);
            cx.notify();
        }
        RowAction::CopyCode { content } => {
            if let Some(code) = editor.session().text().get(content.start..content.end) {
                cx.write_to_clipboard(ClipboardItem::new_string(code.to_owned()));
            }
        }
        RowAction::OpenImage { source } => {
            let (images, index) = note_images(editor, source, cx);
            if !images.is_empty() {
                surfaces::open_lightbox(editor, images, index, window, cx);
            }
        }
        RowAction::PlaceCaret(offset) => {
            editor.session_mut().select(Selection::cursor(offset));
            let focus = editor.focus_handle_ref().clone();
            window.focus(&focus, cx);
            editor.edited(Edited::Selection, window, cx);
        }
        RowAction::OpenPath(path) => open_path(path, window, cx),
        RowAction::JumpToFootnote(id) => {
            let marker = format!("[^{id}]:");
            let document = editor.session().document();
            let found = (0..document.line_count())
                .find(|line| document.line_text(*line).trim_start().starts_with(&marker))
                .map(|line| document.line_span(line).end);
            if let Some(offset) = found {
                editor.session_mut().select(Selection::cursor(offset));
                editor.edited(Edited::Selection, window, cx);
            }
        }
        RowAction::Table { at, op } => {
            let document = editor.session().document();
            if let Some(location) = cell_at(document, at)
                && let Some(change) = apply_table_op(document, &location, op)
            {
                editor.apply(change, EditOrigin::Command, window, cx);
            }
        }
    }
}

fn open_path(path: RelativePath, _window: &mut Window, cx: &mut Context<'_, NoteEditor>) {
    cx.emit(WorkbenchEvent::Open(OpenRequest::new(path)));
}

fn toggle_task(
    editor: &mut NoteEditor,
    marker: Span,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let state = marker.start + 1;
    let Some(current) = editor.session().text().get(state..=state) else {
        return;
    };
    let next = if current == " " { "x" } else { " " };
    let change = Change::new(vec![TextEdit::new(Span::new(state, state + 1), next)]);
    editor.apply(change, EditOrigin::Command, window, cx);
}

fn note_images(editor: &NoteEditor, clicked: Span, cx: &App) -> (Vec<LightboxImage>, usize) {
    let Some(index) = VaultIndex::get(cx) else {
        return (Vec::new(), 0);
    };
    let document = editor.session().document();
    let mut found = Vec::new();
    for block in document.blocks() {
        collect_block(block, &mut found);
    }
    let mut images = Vec::new();
    let mut selected = 0;
    for inline in found {
        let text = inline.span.slice(document.text());
        let resolved = match &inline.kind {
            InlineKind::Image(target) => {
                let label = inline.content.slice(document.text());
                resolve_image(index, editor.note_path(), &target.destination, label)
            }
            InlineKind::Embed(_) => {
                let inner = text.trim_start_matches("![[").trim_end_matches("]]");
                match resolve_embed(index, editor.note_path(), inner) {
                    Some(Embed::Image(image)) => Some(image),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(image) = resolved else {
            continue;
        };
        if inline.span.overlaps(clicked.start, clicked.end) {
            selected = images.len();
        }
        let source = match image.location {
            ImageLocation::Local(path) => ImageSource::from(path),
            ImageLocation::Remote(url) => ImageSource::from(SharedString::from(url)),
        };
        images.push(LightboxImage {
            source,
            alt: SharedString::from(image.alt),
        });
    }
    (images, selected)
}

fn collect_block<'a>(block: &'a Block, out: &mut Vec<&'a Inline>) {
    for inline in &block.inlines {
        collect_inline(inline, out);
    }
    if let Some(table) = block.table() {
        for row in &table.rows {
            for cell in &row.cells {
                for inline in &cell.inlines {
                    collect_inline(inline, out);
                }
            }
        }
    }
    for child in &block.children {
        collect_block(child, out);
    }
}

fn collect_inline<'a>(inline: &'a Inline, out: &mut Vec<&'a Inline>) {
    if matches!(inline.kind, InlineKind::Image(_) | InlineKind::Embed(_)) {
        out.push(inline);
    }
    for child in &inline.children {
        collect_inline(child, out);
    }
}

#[cfg(test)]
mod tests {
    use margin_markdown::Document;

    use super::*;

    #[test]
    fn collects_images_from_nested_blocks() {
        let document = Document::parse("> ![a](a.png)\n\n- ![[b.png]]\n");
        let mut found = Vec::new();
        for block in document.blocks() {
            collect_block(block, &mut found);
        }
        assert_eq!(found.len(), 2);
    }
}
