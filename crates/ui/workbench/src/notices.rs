use std::fmt::Display;

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, Window};
use margin_i18n::{t, t_with};

pub fn notify_error(window: &mut Window, cx: &mut App, message_key: &str, error: &impl Display) {
    let detail = error.to_string();
    let message = t_with(cx, message_key, &[("error", detail.as_str())]);
    window.push_notification(Notification::error(message), cx);
}

pub fn notify_info(window: &mut Window, cx: &mut App, message_key: &str) {
    let message = t(cx, message_key);
    window.push_notification(Notification::info(message), cx);
}
