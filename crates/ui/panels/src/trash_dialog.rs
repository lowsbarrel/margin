use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::WindowExt;
use gpui_kit::{App, AppContext, ParentElement, Window, rems};
use margin_i18n::t;
use margin_workbench::WorkbenchEvent;

use crate::trash_view::TrashView;

const DIALOG_WIDTH_REMS: f32 = 40.0;

pub fn open_trash_dialog(
    window: &mut Window,
    cx: &mut App,
    on_event: impl Fn(WorkbenchEvent, &mut Window, &mut App) + 'static,
) {
    let view = cx.new(|cx| TrashView::new(Rc::new(on_event), window, cx));
    let title = t(cx, "trash_title");
    let width = rems(DIALOG_WIDTH_REMS).to_pixels(window.rem_size());
    let focus_search = Cell::new(true);
    window.open_dialog(cx, move |dialog, window, cx| {
        if focus_search.replace(false) {
            let search = view.read(cx).search();
            search.update(cx, |search, cx| search.focus(window, cx));
        }
        dialog.title(title.clone()).w(width).child(view.clone())
    });
}
