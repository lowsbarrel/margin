use gpui_kit::{AnyWindowHandle, Context, Task, Window};

use crate::note_editor::NoteEditor;

pub(crate) fn after<T: 'static>(
    task: Task<T>,
    window: AnyWindowHandle,
    cx: &Context<'_, NoteEditor>,
    apply: impl FnOnce(&mut NoteEditor, T, &mut Window, &mut Context<'_, NoteEditor>) + 'static,
) {
    cx.spawn(async move |this, cx| {
        let value = task.await;
        drop(window.update(cx, |_, window, cx| {
            drop(this.update(cx, |editor, cx| apply(editor, value, window, cx)));
        }));
    })
    .detach();
}
