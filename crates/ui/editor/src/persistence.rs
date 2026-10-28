use std::sync::Arc;

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::{Context, Window};
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_text_buffer::{Selection, floor_char_boundary};
use margin_workbench::{WorkbenchEvent, notify_error, run_blocking};

use crate::note_editor::{Edited, NoteEditor};
use crate::save::{self, FinishResult, Outcome};
use crate::session::Session;
use crate::{async_ops, surfaces};

pub(crate) fn load(editor: &NoteEditor, cx: &Context<'_, NoteEditor>) {
    let services = editor.services();
    let path = editor.note_path().clone();
    let read = run_blocking(cx, move || services.files.read_file(&path));
    async_ops::after(
        read,
        editor.window_handle(),
        cx,
        |editor, result, window, cx| match result {
            Ok(bytes) => adopt_loaded(editor, &String::from_utf8_lossy(&bytes), window, cx),
            Err(error) => notify_error(window, cx, "toast_cannot_open_file", &error),
        },
    );
}

fn adopt_loaded(
    editor: &mut NoteEditor,
    text: &str,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let session = Session::new(text, editor.view_session_mode());
    let revision = session.buffer().revision();
    let (cursor, anchor) = editor.mark_loaded(session);
    editor.save_mut().loaded(Arc::from(text), revision);
    if let Some(offset) = cursor {
        let offset = floor_char_boundary(text, offset.min(text.len()));
        editor.session_mut().select(Selection::cursor(offset));
        editor.reveal(offset);
    }
    if let Some(anchor) = anchor {
        editor.scroll_to_anchor(&anchor, window, cx);
    }
    surfaces::after_edit(editor, Edited::Text, window, cx);
    cx.notify();
}

pub(crate) fn flush(editor: &mut NoteEditor, cx: &Context<'_, NoteEditor>) {
    if !editor.is_loaded() {
        return;
    }
    let revision = editor.session().buffer().revision();
    let Some(job) = editor.save_mut().begin(revision) else {
        return;
    };
    let text: Arc<str> = Arc::from(editor.session().text());
    save::write(
        job,
        editor.services(),
        editor.note_path().clone(),
        text,
        editor.window_handle(),
        cx,
    );
}

pub(crate) fn finish_save(
    editor: &mut NoteEditor,
    outcome: Outcome,
    snapshot_failed: bool,
    path: &RelativePath,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if snapshot_failed {
        let message = t(cx, "toast_save_snapshot_failed");
        window.push_notification(Notification::error(message), cx);
    }
    match editor.save_mut().finish(outcome) {
        FinishResult::Saved { retry } => {
            cx.emit(WorkbenchEvent::Saved(path.clone()));
            if retry {
                flush(editor, cx);
            }
        }
        FinishResult::Failed { error } => {
            notify_error(window, cx, "toast_save_failed", &error);
        }
    }
    cx.notify();
}

pub(crate) fn reload(editor: &NoteEditor, cx: &Context<'_, NoteEditor>) {
    let revision = editor.session().buffer().revision();
    if !editor.is_loaded() || editor.save_state().has_unsaved_edits(revision) {
        return;
    }
    let services = editor.services();
    let path = editor.note_path().clone();
    let read = run_blocking(cx, move || services.files.read_file(&path));
    async_ops::after(
        read,
        editor.window_handle(),
        cx,
        |editor, result, window, cx| {
            let Ok(bytes) = result else {
                return;
            };
            let revision = editor.session().buffer().revision();
            if editor.save_state().is_dirty(revision) {
                return;
            }
            let text = String::from_utf8_lossy(&bytes);
            if text == editor.session().text() {
                return;
            }
            let current: Arc<str> = Arc::from(editor.session().text());
            if let Some(departing) = editor.save_mut().snapshot_before_adopt(current) {
                save::snapshot(editor.services(), editor.note_path().clone(), departing, cx)
                    .detach();
            }
            editor.session_mut().replace_all(&text);
            let revision = editor.session().buffer().revision();
            editor.save_mut().loaded(Arc::from(&*text), revision);
            surfaces::after_edit(editor, Edited::Text, window, cx);
            cx.notify();
        },
    );
}
