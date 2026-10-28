use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, Sizable};
use gpui_kit::{App, Hsla, KeyDownEvent, Keystroke, SharedString, Styled, px};
use margin_theme::css_px;

pub(crate) const LINE_STEP: f32 = 40.0;
pub(crate) const PAGE_FRACTION: f32 = 0.875;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomKey {
    In,
    Out,
    Actual,
    Fit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ScrollKey {
    Line(f32, f32),
    Page(f32),
    Start,
    End,
}

#[must_use]
pub fn zoom_key(event: &KeyDownEvent, _cx: &App) -> Option<ZoomKey> {
    zoom_keystroke(&event.keystroke)
}

fn zoom_keystroke(keystroke: &Keystroke) -> Option<ZoomKey> {
    let modifiers = keystroke.modifiers;
    if modifiers.control || modifiers.platform || modifiers.alt || modifiers.function {
        return None;
    }
    match keystroke.key.as_str() {
        "=" | "+" => Some(ZoomKey::In),
        "-" | "_" => Some(ZoomKey::Out),
        "0" => Some(ZoomKey::Actual),
        "f" if !modifiers.shift => Some(ZoomKey::Fit),
        _ => None,
    }
}

#[must_use]
pub(crate) fn scroll_key(event: &KeyDownEvent) -> Option<ScrollKey> {
    let modifiers = event.keystroke.modifiers;
    if modifiers.control || modifiers.platform || modifiers.alt {
        return None;
    }
    let back = if modifiers.shift { -1.0 } else { 1.0 };
    match event.keystroke.key.as_str() {
        "up" => Some(ScrollKey::Line(0.0, -1.0)),
        "down" => Some(ScrollKey::Line(0.0, 1.0)),
        "left" => Some(ScrollKey::Line(-1.0, 0.0)),
        "right" => Some(ScrollKey::Line(1.0, 0.0)),
        "pageup" => Some(ScrollKey::Page(-1.0)),
        "pagedown" => Some(ScrollKey::Page(1.0)),
        "space" => Some(ScrollKey::Page(back)),
        "home" => Some(ScrollKey::Start),
        "end" => Some(ScrollKey::End),
        _ => None,
    }
}

#[must_use]
pub(crate) fn icon_button(
    id: &'static str,
    icon: IconName,
    tooltip: SharedString,
    rest: Hsla,
) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .rounded(px(4.))
        .w(css_px(24.5))
        .h(css_px(22.75))
        .text_color(rest)
        .icon(Icon::new(icon).size(css_px(14.0)))
        .tooltip(tooltip.clone())
        .accessibility_label(tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &'static str) -> Option<ZoomKey> {
        zoom_keystroke(&Keystroke::parse(text).expect("valid keystroke"))
    }

    #[test]
    fn zoom_keys_ignore_modified_presses_so_app_zoom_keeps_working() {
        assert_eq!(key("="), Some(ZoomKey::In));
        assert_eq!(key("shift-="), Some(ZoomKey::In));
        assert_eq!(key("-"), Some(ZoomKey::Out));
        assert_eq!(key("shift-_"), Some(ZoomKey::Out));
        assert_eq!(key("0"), Some(ZoomKey::Actual));
        assert_eq!(key("f"), Some(ZoomKey::Fit));
        assert_eq!(key("shift-f"), None);
        assert_eq!(key("cmd-="), None);
        assert_eq!(key("ctrl--"), None);
        assert_eq!(key("cmd-0"), None);
        assert_eq!(key("alt-f"), None);
        assert_eq!(key("g"), None);
    }
}
