use gpui_kit::Context;
use margin_kernel::RelativePath;
use margin_workbench::TabContent;

use crate::note_editor::NoteEditor;

impl TabContent for NoteEditor {
    fn path(&self) -> &RelativePath {
        self.note_path()
    }

    fn rename(&mut self, to: RelativePath, cx: &mut Context<'_, Self>) {
        let window = self.window_handle();
        self.title_row().sync_to_path(&to, window, cx);
        self.set_path(to, cx);
    }

    fn flush(&mut self, cx: &mut Context<'_, Self>) {
        self.flush_now(cx);
    }

    fn reload(&mut self, cx: &mut Context<'_, Self>) {
        self.reload_from_disk(cx);
    }
}
