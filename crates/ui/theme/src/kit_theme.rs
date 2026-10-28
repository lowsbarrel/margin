use std::fmt::Write;

use gpui_kit::{Hsla, Rgba};
use serde_json::{Map, Value, json};

use crate::fonts::{MONO_FONT_FAMILY, UI_FONT_FAMILY};
use crate::palette::{CalloutTone, Palette};

pub(crate) const LIGHT_NAME: &str = "Margin Light";
pub(crate) const DARK_NAME: &str = "Margin Dark";
pub(crate) const UI_FONT_SIZE: f32 = 14.0;
pub(crate) const MONO_FONT_SIZE: f32 = 13.0;

pub(crate) fn theme_set() -> String {
    json!({
        "name": "Margin",
        "author": "Margin",
        "themes": [theme(LIGHT_NAME, &Palette::light()), theme(DARK_NAME, &Palette::dark())],
    })
    .to_string()
}

fn theme(name: &str, palette: &Palette) -> Value {
    json!({
        "is_default": true,
        "name": name,
        "mode": if palette.dark { "dark" } else { "light" },
        "font.family": UI_FONT_FAMILY,
        "font.size": UI_FONT_SIZE,
        "mono_font.family": MONO_FONT_FAMILY,
        "mono_font.size": MONO_FONT_SIZE,
        "radius": 8,
        "radius.lg": 12,
        "shadow": true,
        "colors": Value::Object(colors(palette)),
    })
}

fn colors(palette: &Palette) -> Map<String, Value> {
    let white = palette.text_inverse;
    let on_brand = if palette.dark {
        palette.text_inverse
    } else {
        palette.surface(0)
    };
    let pairs: [(&str, Hsla); 72] = [
        ("background", palette.surface(0)),
        ("foreground", palette.text_primary),
        ("border", palette.border_divider),
        ("input.border", palette.border_strong),
        ("ring", palette.bg_brand),
        ("caret", palette.text_primary),
        ("selection.background", palette.brand(32.0)),
        ("overlay", palette.bg_overlay),
        ("window.border", palette.border_divider),
        ("muted.background", palette.surface(2)),
        ("muted.foreground", palette.text_secondary),
        ("accent.background", palette.surface(3)),
        ("accent.foreground", palette.text_primary),
        ("primary.background", palette.bg_brand),
        ("primary.hover.background", palette.bg_brand_hover),
        ("primary.active.background", palette.bg_brand_hover),
        ("primary.foreground", on_brand),
        ("secondary.background", palette.surface(2)),
        ("secondary.hover.background", palette.surface(3)),
        ("secondary.active.background", palette.surface(3)),
        ("secondary.foreground", palette.text_primary),
        ("danger.background", palette.bg_negative),
        ("danger.hover.background", palette.bg_negative_hover),
        ("danger.active.background", palette.bg_negative_hover),
        ("danger.foreground", on_brand),
        ("success.background", palette.bg_positive),
        ("success.hover.background", palette.bg_positive),
        ("success.active.background", palette.bg_positive),
        ("success.foreground", on_brand),
        ("warning.background", palette.bg_warning),
        ("warning.hover.background", palette.bg_warning),
        ("warning.active.background", palette.bg_warning),
        ("warning.foreground", palette.text_primary),
        ("info.background", palette.callout(CalloutTone::Info)),
        ("info.hover.background", palette.callout(CalloutTone::Info)),
        ("info.active.background", palette.callout(CalloutTone::Info)),
        ("info.foreground", on_brand),
        ("link", palette.text_brand),
        ("link.hover", palette.bg_brand_hover),
        ("link.active", palette.bg_brand_hover),
        ("popover.background", palette.surface(0)),
        ("popover.foreground", palette.text_primary),
        ("list.background", palette.surface(0)),
        ("list.even.background", palette.surface(0)),
        ("list.head.background", palette.surface(1)),
        ("list.hover.background", palette.surface(3)),
        ("list.active.background", palette.brand(16.0)),
        ("list.active.border", palette.brand(0.0)),
        ("table.background", palette.surface(0)),
        ("table.head.background", palette.surface(2)),
        ("table.head.foreground", palette.text_primary),
        ("table.hover.background", palette.surface(3)),
        ("table.active.background", palette.brand(16.0)),
        ("table.row.border", palette.border_divider),
        ("sidebar.background", palette.surface(1)),
        ("sidebar.foreground", palette.text_primary),
        ("sidebar.border", palette.border_divider),
        ("sidebar.accent.background", palette.surface(3)),
        ("sidebar.accent.foreground", palette.text_primary),
        ("sidebar.primary.background", palette.bg_brand),
        ("sidebar.primary.foreground", white),
        ("tab.background", palette.surface(1)),
        ("tab.active.background", palette.surface(0)),
        ("tab.active.foreground", palette.text_primary),
        ("tab.foreground", palette.text_secondary),
        ("tab_bar.background", palette.surface(1)),
        ("title_bar.background", palette.surface(1)),
        ("title_bar.border", palette.border_divider),
        ("status_bar.background", palette.surface(1)),
        ("status_bar.border", palette.border_divider),
        ("drag.border", palette.bg_brand),
        ("drop_target.background", palette.brand(8.0)),
    ];
    let extra: [(&str, Hsla); 6] = [
        ("scrollbar.background", palette.brand(0.0)),
        ("scrollbar.thumb.background", palette.border_strong),
        ("scrollbar.thumb.hover.background", palette.text_tertiary),
        ("skeleton.background", palette.surface(2)),
        ("switch.background", palette.border_strong),
        ("progress.bar.background", palette.bg_brand),
    ];
    pairs
        .into_iter()
        .chain(extra)
        .map(|(key, color)| (key.to_owned(), Value::String(hex(color))))
        .collect()
}

fn hex(color: Hsla) -> String {
    let rgba = Rgba::from(color);
    let mut text = String::from("#");
    for channel in [rgba.r, rgba.g, rgba.b, rgba.a] {
        write!(text, "{:02x}", channel_byte(channel)).expect("writing to a String never fails");
    }
    text
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the channel is clamped to 0..=1 and rounded, so the product fits a byte"
)]
fn channel_byte(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encodes_rgba_channels() {
        assert_eq!(hex(gpui_kit::hsla(0.0, 0.0, 1.0, 1.0)), "#ffffffff");
        assert_eq!(hex(gpui_kit::hsla(0.0, 0.0, 0.0, 0.5)), "#00000080");
    }

    #[test]
    fn theme_set_parses_as_two_default_themes() {
        let set: Value = serde_json::from_str(&theme_set()).expect("the theme set is valid JSON");
        let themes = set["themes"].as_array().expect("themes is an array");
        assert_eq!(themes.len(), 2);
        assert!(
            themes
                .iter()
                .all(|theme| theme["is_default"] == Value::Bool(true))
        );
        assert_eq!(themes[1]["mode"], "dark");
    }
}
