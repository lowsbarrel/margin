use std::path::PathBuf;

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, ClipboardEntry, ClipboardItem, Context, Global, ImageFormat, Window};
use margin_editor_assets::{AttachmentFailure, AttachmentSource, VaultIndex, store_attachments};
pub(crate) use margin_editor_commands::PasteMode;
use margin_editor_commands::{insert_text, paste as paste_change};
use margin_i18n::t_with;
use margin_markdown::{Span, TextEdit};
use margin_text_buffer::{Change, EditOrigin, Selection, line_span};
use margin_workbench::WorkbenchEvent;

use crate::note_editor::NoteEditor;
use crate::{async_ops, editing};

fn copied_text(editor: &NoteEditor) -> (String, Vec<Span>) {
    let text = editor.session().text();
    let selections = editor.session().selections();
    if selections.iter().all(Selection::is_empty) {
        let mut spans: Vec<Span> = Vec::new();
        for selection in selections {
            let line = line_span(text, selection.head);
            let end = if line.end < text.len() {
                line.end + 1
            } else {
                line.end
            };
            let span = Span::new(line.start, end);
            if spans.last() != Some(&span) {
                spans.push(span);
            }
        }
        let copied = spans
            .iter()
            .map(|span| span.slice(text))
            .collect::<String>();
        return (copied, spans);
    }
    let spans: Vec<Span> = selections
        .iter()
        .filter(|selection| !selection.is_empty())
        .map(Selection::span)
        .collect();
    let copied = spans
        .iter()
        .map(|span| span.slice(text))
        .collect::<Vec<_>>()
        .join("\n");
    (copied, spans)
}

#[derive(Default)]
struct LinewiseCopy(String);

impl Global for LinewiseCopy {}

fn write_copied(linewise: bool, copied: String, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()));
    cx.set_global(LinewiseCopy(if linewise { copied } else { String::new() }));
}

pub(crate) fn copy(editor: &NoteEditor, cx: &mut Context<'_, NoteEditor>) {
    let (copied, _) = copied_text(editor);
    write_copied(is_linewise(editor), copied, cx);
}

pub(crate) fn cut(editor: &mut NoteEditor, window: &Window, cx: &mut Context<'_, NoteEditor>) {
    let (copied, spans) = copied_text(editor);
    write_copied(is_linewise(editor), copied, cx);
    let edits = spans
        .into_iter()
        .map(|span| TextEdit::new(span, ""))
        .collect();
    editor.apply(Change::new(edits), EditOrigin::Deletion, window, cx);
}

fn is_linewise(editor: &NoteEditor) -> bool {
    editor
        .session()
        .selections()
        .iter()
        .all(Selection::is_empty)
}

fn paste_linewise(
    editor: &mut NoteEditor,
    plain: &str,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) -> bool {
    let copied = cx.try_global::<LinewiseCopy>().map(|copy| copy.0.as_str());
    if copied != Some(plain) || !is_linewise(editor) {
        return false;
    }
    let text = editor.session().text();
    let mut edits: Vec<TextEdit> = Vec::new();
    let mut cursors = Vec::new();
    for selection in editor.session().selections() {
        let start = line_span(text, selection.head).start;
        if edits.last().is_none_or(|edit| edit.span.start != start) {
            edits.push(TextEdit::new(Span::empty(start), plain));
        }
        cursors.push(Selection::cursor(
            selection.head + plain.len() * edits.len(),
        ));
    }
    let change = Change::new(edits).with_selections(cursors);
    editor.apply(change, EditOrigin::Paste, window, cx);
    true
}

pub(crate) fn paste(
    editor: &mut NoteEditor,
    mode: PasteMode,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let Some(item) = cx.read_from_clipboard() else {
        return;
    };
    let mut sources = Vec::new();
    let mut plain = String::new();
    for entry in item.entries() {
        match entry {
            ClipboardEntry::String(string) => plain.push_str(string.text()),
            ClipboardEntry::Image(image) => sources.push(AttachmentSource::Bytes {
                bytes: image.bytes.clone(),
                name: None,
                extension: extension(image.format).to_owned(),
            }),
            ClipboardEntry::ExternalPaths(paths) => {
                sources.extend(paths.paths().iter().cloned().map(AttachmentSource::Path));
            }
        }
    }
    if !sources.is_empty() {
        insert_attachments(editor, sources, Failure::Paste, cx);
        return;
    }
    if plain.is_empty() {
        return;
    }
    if paste_linewise(editor, &plain, window, cx) {
        return;
    }
    let pasted = editing::apply_change(
        editor,
        EditOrigin::Paste,
        window,
        cx,
        |document, selections, primary| {
            paste_change(document, selections, primary, &plain, None, mode)
        },
    );
    if !pasted {
        editing::apply_change(
            editor,
            EditOrigin::Paste,
            window,
            cx,
            |document, selections, _| Some(insert_text(document, selections, &plain)),
        );
    }
}

pub(crate) fn drop_paths(editor: &NoteEditor, paths: Vec<PathBuf>, cx: &Context<'_, NoteEditor>) {
    let sources = paths.into_iter().map(AttachmentSource::Path).collect();
    insert_attachments(editor, sources, Failure::Drop, cx);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Paste,
    Drop,
}

fn insert_attachments(
    editor: &NoteEditor,
    sources: Vec<AttachmentSource>,
    failure: Failure,
    cx: &Context<'_, NoteEditor>,
) {
    let task = store_attachments(sources, cx);
    async_ops::after(
        task,
        editor.window_handle(),
        cx,
        move |editor, results, window, cx| {
            let mut embeds = Vec::new();
            for result in results {
                match result {
                    Ok(embed) => embeds.push(embed),
                    Err(error) => report(&error, failure, window, cx),
                }
            }
            if embeds.is_empty() {
                return;
            }
            let text = embeds.concat();
            editing::apply_change(
                editor,
                EditOrigin::Paste,
                window,
                cx,
                |document, selections, _| Some(insert_text(document, selections, &text)),
            );
            VaultIndex::refresh(cx);
            cx.emit(WorkbenchEvent::TreeChanged);
        },
    );
}

fn report(
    error: &AttachmentFailure,
    failure: Failure,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let detail = error.error.to_string();
    let message = match failure {
        Failure::Paste => t_with(
            cx,
            "toast_attachment_paste_failed",
            &[("name", error.name.as_str()), ("error", detail.as_str())],
        ),
        Failure::Drop => t_with(
            cx,
            "toast_insert_file_failed",
            &[("error", detail.as_str())],
        ),
    };
    window.push_notification(Notification::error(message), cx);
}

const fn extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png | ImageFormat::Pnm => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Ico => "ico",
    }
}
