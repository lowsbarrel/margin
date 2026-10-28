use std::rc::Rc;

use gpui_kit::component::WindowExt;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::{App, SharedString, Window};
use margin_i18n::t;

pub(crate) fn confirm_dialog(
    window: &mut Window,
    cx: &mut App,
    [title, body, confirm]: [SharedString; 3],
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
) {
    let cancel = t(cx, "panels_cancel");
    let on_confirm = Rc::new(on_confirm);
    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_confirm = Rc::clone(&on_confirm);
        alert
            .title(title.clone())
            .description(body.clone())
            .ok_text(confirm.clone())
            .ok_variant(ButtonVariant::Danger)
            .cancel_text(cancel.clone())
            .show_cancel(true)
            .on_ok(move |_, window, cx| {
                on_confirm(window, cx);
                true
            })
    });
}
