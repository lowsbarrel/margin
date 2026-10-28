use std::time::Duration;

use gpui_kit::{Context, Task, Window};

use crate::note_editor::NoteEditor;

const BLINK_PHASE: Duration = Duration::from_millis(600);

#[derive(Default)]
pub(crate) struct CaretBlink {
    hidden: bool,
    ticking: Option<Task<()>>,
}

impl CaretBlink {
    pub(crate) fn restart(&mut self) {
        self.hidden = false;
        self.ticking = None;
    }

    pub(crate) const fn is_visible(&self) -> bool {
        !self.hidden
    }

    pub(crate) fn update(
        editor: &mut NoteEditor,
        focused: bool,
        window: &Window,
        cx: &Context<'_, NoteEditor>,
    ) {
        if !focused || cx.reduce_motion() {
            editor.render_state_mut().caret_mut().restart();
            return;
        }
        if editor.render_state_mut().caret_mut().ticking.is_some() {
            return;
        }
        let task = cx.spawn_in(window, async move |editor, cx| {
            loop {
                cx.background_executor().timer(BLINK_PHASE).await;
                let toggled = editor.update(cx, |editor, cx| {
                    let blink = editor.render_state_mut().caret_mut();
                    blink.hidden = !blink.hidden;
                    cx.notify();
                });
                if toggled.is_err() {
                    break;
                }
            }
        });
        editor.render_state_mut().caret_mut().ticking = Some(task);
    }
}
