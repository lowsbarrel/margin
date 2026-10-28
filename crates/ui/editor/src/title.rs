use std::time::Duration;

use gpui_kit::component::WindowExt;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::{
    AnyWindowHandle, AppContext, Context, Entity, Focusable, KeyDownEvent, Subscription, Task,
    Window,
};
use margin_i18n::{t, t_with};
use margin_kernel::RelativePath;
use margin_workbench::{WorkbenchEvent, notify_error, run_blocking};

use crate::note_editor::NoteEditor;
use crate::{async_ops, completion};

const RENAME_DELAY: Duration = Duration::from_millis(150);
const INVALID_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Report {
    Silent,
    Notify,
}

pub(crate) struct TitleRow {
    input: Entity<InputState>,
    timer: Option<Task<()>>,
    pending: bool,
    queued: Option<String>,
    _subscription: Subscription,
}

pub(crate) fn file_title(path: &RelativePath) -> String {
    let name = path.file_name();
    name.strip_suffix(".md")
        .or_else(|| name.strip_suffix(".canvas"))
        .unwrap_or(name)
        .to_owned()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NameError {
    Empty,
    Ascii,
    InvalidChars,
    DotStart,
    Trailing,
    Reserved,
}

pub(crate) fn validate_name(name: &str) -> Result<&str, NameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(NameError::Empty);
    }
    if trimmed.chars().any(|c| !(' '..='~').contains(&c)) {
        return Err(NameError::Ascii);
    }
    if trimmed.contains(INVALID_CHARS) {
        return Err(NameError::InvalidChars);
    }
    if trimmed.starts_with('.') {
        return Err(NameError::DotStart);
    }
    if trimmed.ends_with(|c: char| c.is_whitespace() || c == '.') {
        return Err(NameError::Trailing);
    }
    let stem = trimmed.rsplit_once('.').map_or(trimmed, |(stem, _)| stem);
    if RESERVED_NAMES.contains(&stem.to_ascii_uppercase().as_str()) {
        return Err(NameError::Reserved);
    }
    Ok(trimmed)
}

impl TitleRow {
    pub(crate) fn new(
        path: &RelativePath,
        window: &mut Window,
        cx: &mut Context<'_, NoteEditor>,
    ) -> Self {
        let title = file_title(path);
        let placeholder = t(cx, "editor_untitled");
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(placeholder);
            state.set_value(title, window, cx);
            state
        });
        let subscription = cx.subscribe_in(&input, window, |editor, _, event, window, cx| {
            on_event(editor, event, window, cx);
        });
        Self {
            input,
            timer: None,
            pending: false,
            queued: None,
            _subscription: subscription,
        }
    }

    pub(crate) const fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn sync_to_path(
        &self,
        path: &RelativePath,
        window: AnyWindowHandle,
        cx: &mut Context<'_, NoteEditor>,
    ) {
        if self.pending {
            return;
        }
        let title = file_title(path);
        let input = self.input.clone();
        drop(window.update(cx, |_, window, cx| {
            input.update(cx, |state, cx| {
                if !state.focus_handle(cx).is_focused(window) {
                    state.set_value(title, window, cx);
                }
            });
        }));
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &mut Context<'_, NoteEditor>) {
        self.input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
    }
}

pub(crate) fn key_down(
    editor: &mut NoteEditor,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if event.keystroke.modifiers.modified() {
        return;
    }
    match event.keystroke.key.as_str() {
        "/" => {
            cx.stop_propagation();
            completion::open_slash_menu(editor, window, cx);
        }
        "down" => {
            cx.stop_propagation();
            window.focus(&editor.focus_handle_ref().clone(), cx);
        }
        _ => {}
    }
}

fn on_event(
    editor: &mut NoteEditor,
    event: &InputEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    match event {
        InputEvent::Change => {
            let raw = editor.title_row().input.read(cx).value().trim().to_owned();
            if raw.is_empty() {
                editor.title_row_mut().timer = None;
                return;
            }
            let handle = editor.window_handle();
            editor.title_row_mut().timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(RENAME_DELAY).await;
                drop(handle.update(cx, |_, window, cx| {
                    drop(this.update(cx, |editor, cx| {
                        editor.title_row_mut().timer = None;
                        request(editor, raw, Report::Notify, window, cx);
                    }));
                }));
            }));
        }
        InputEvent::Blur => {
            let debounced = editor.title_row_mut().timer.take().is_some();
            let name = editor.title_row().input.read(cx).value().trim().to_owned();
            let report = if debounced {
                Report::Notify
            } else {
                Report::Silent
            };
            request(editor, name, report, window, cx);
            if !editor.title_row().pending {
                let title = file_title(editor.note_path());
                editor
                    .title_row()
                    .input
                    .update(cx, |state, cx| state.set_value(title, window, cx));
            }
        }
        InputEvent::PressEnter { .. } => {
            window.focus(&editor.focus_handle_ref().clone(), cx);
        }
        InputEvent::Focus => {}
    }
}

fn request(
    editor: &mut NoteEditor,
    name: String,
    report: Report,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if name.is_empty() {
        return;
    }
    match validate_name(&name) {
        Ok(_) => {
            editor.title_row_mut().queued = Some(name);
            drain(editor, cx);
        }
        Err(error) => {
            if report == Report::Notify {
                let message = match error {
                    NameError::Empty => t(cx, "validate_name_empty"),
                    NameError::Ascii => t(cx, "validate_name_ascii"),
                    NameError::InvalidChars => t(cx, "validate_name_invalid_chars"),
                    NameError::DotStart => t(cx, "validate_name_dot_start"),
                    NameError::Trailing => t(cx, "validate_name_trailing"),
                    NameError::Reserved => {
                        t_with(cx, "validate_name_reserved", &[("name", name.as_str())])
                    }
                };
                notify_error(window, cx, "toast_rename_failed", &message);
            }
        }
    }
}

fn drain(editor: &mut NoteEditor, cx: &Context<'_, NoteEditor>) {
    if editor.title_row().pending {
        return;
    }
    let Some(name) = editor.title_row_mut().queued.take() else {
        return;
    };
    let from = editor.note_path().clone();
    if name == file_title(&from) {
        return;
    }
    let Ok(to) = from.with_file_name(&format!("{name}.md")) else {
        return;
    };
    editor.title_row_mut().pending = true;
    editor.flush_now(cx);
    let services = editor.services();
    let source = from.clone();
    let target = to.clone();
    let work = run_blocking(cx, move || {
        let differs_beyond_case = source.as_str().to_lowercase() != target.as_str().to_lowercase();
        if differs_beyond_case && services.files.exists(&target) {
            return Err(RenameError::Exists);
        }
        services
            .files
            .rename_entry(&source, &target)
            .map_err(|error| RenameError::Failed(error.to_string()))?;
        drop(services.history.rename(&source, &target));
        Ok(())
    });
    async_ops::after(
        work,
        editor.window_handle(),
        cx,
        move |editor, result, window, cx| {
            editor.title_row_mut().pending = false;
            match result {
                Ok(()) => {
                    editor.set_path(to.clone(), cx);
                    cx.emit(WorkbenchEvent::Renamed { from, to });
                }
                Err(RenameError::Exists) => {
                    let message = t_with(cx, "toast_path_exists", &[("name", to.file_name())]);
                    window.push_notification(Notification::error(message), cx);
                }
                Err(RenameError::Failed(error)) => {
                    notify_error(window, cx, "toast_rename_failed", &error);
                }
            }
            if editor.title_row().queued.is_some() {
                drain(editor, cx);
            }
        },
    );
}

enum RenameError {
    Exists,
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_like_the_original() {
        assert_eq!(validate_name("  Plan  "), Ok("Plan"));
        assert_eq!(validate_name(""), Err(NameError::Empty));
        assert_eq!(validate_name("a/b"), Err(NameError::InvalidChars));
        assert_eq!(validate_name(".hidden"), Err(NameError::DotStart));
        assert_eq!(validate_name("end."), Err(NameError::Trailing));
        assert_eq!(validate_name("con"), Err(NameError::Reserved));
        assert_eq!(validate_name("caffè"), Err(NameError::Ascii));
    }
}
