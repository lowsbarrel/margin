#![expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui_kit::actions! derives PartialEq without Eq"
)]

use gpui_kit::component::WindowExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, Window, actions};
use margin_i18n::{t, t_with};
use margin_workbench::{AvailableUpdate, Updates};

actions!(margin_app, [RestartAfterUpdate]);

struct UpdateNotice;

pub(crate) fn check(window: &mut Window, cx: &mut App) {
    if cx.try_global::<Updates>().is_none() {
        return;
    }
    let checking = Updates::check(cx);
    window
        .spawn(cx, async move |cx| {
            if let Ok(Some(update)) = checking.await {
                drop(cx.update(|window, cx| announce(update, window, cx)));
            }
        })
        .detach();
}

fn announce(update: AvailableUpdate, window: &mut Window, cx: &mut App) {
    let message = t_with(
        cx,
        "update_available",
        &[("version", update.version.as_str())],
    );
    window.remove_notification::<UpdateNotice>(cx);
    let notice = Notification::info(message)
        .id::<UpdateNotice>()
        .action(move |_, _, cx| {
            let update = update.clone();
            Button::new("install-update")
                .primary()
                .label(t(cx, "update_btn"))
                .on_click(move |_, window, cx| install(update.clone(), window, cx))
        });
    window.push_notification(notice, cx);
}

fn install(update: AvailableUpdate, window: &mut Window, cx: &mut App) {
    window.remove_notification::<UpdateNotice>(cx);
    window.push_notification(
        Notification::info(t(cx, "update_installing")).id::<UpdateNotice>(),
        cx,
    );
    let installing = Updates::install(update, cx);
    window
        .spawn(cx, async move |cx| {
            let installed = installing.await;
            drop(cx.update(|window, cx| {
                window.remove_notification::<UpdateNotice>(cx);
                if installed.is_ok() {
                    window.dispatch_action(Box::new(RestartAfterUpdate), cx);
                } else {
                    window.push_notification(Notification::error(t(cx, "toast_update_failed")), cx);
                }
            }));
        })
        .detach();
}
