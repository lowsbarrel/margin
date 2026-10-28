use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{App, Div, InteractiveElement, ParentElement, SharedString, Styled, div};
use margin_i18n::{t_count, t_with};
use margin_kernel::RelativePath;
use margin_theme::MarginTheme;

use crate::vault_files::file_size;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbedKind {
    Note,
    File,
}

#[must_use]
pub fn embed_icon(kind: EmbedKind) -> Icon {
    Icon::new(match kind {
        EmbedKind::Note => IconName::FileText,
        EmbedKind::File => IconName::Paperclip,
    })
}

#[must_use]
pub fn file_card(name: &str, path: Option<&RelativePath>, cx: &mut App) -> Div {
    let detail = match path {
        Some(path) => file_size(path, cx).map(|bytes| format_size(bytes, cx)),
        None => Some(t_with(cx, "file_embed_missing", &[("name", name)])),
    };
    let palette = MarginTheme::palette(cx);
    let card = div()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .py_0p5()
        .my_0p5()
        .rounded_lg()
        .border_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface[1])
        .child(embed_icon(EmbedKind::File))
        .child(SharedString::from(name.to_owned()))
        .children(detail.map(|detail| {
            div()
                .text_color(palette.text_tertiary)
                .text_sm()
                .child(detail)
        }));
    if path.is_some() {
        card.text_color(palette.text_primary)
            .cursor_pointer()
            .hover(|style| style.bg(palette.surface[2]))
    } else {
        card.text_color(palette.text_tertiary)
    }
}

fn format_size(bytes: u64, cx: &App) -> SharedString {
    if bytes < 1024 {
        return t_count(cx, "file_info_size_bytes", bytes, &[]);
    }
    #[expect(clippy::cast_precision_loss, reason = "a rounded display size")]
    let kilobytes = bytes as f64 / 1024.0;
    if kilobytes < 1024.0 {
        return t_with(cx, "file_info_size_kb", &[("size", &rounded(kilobytes))]);
    }
    t_with(
        cx,
        "file_info_size_mb",
        &[("size", &rounded(kilobytes / 1024.0))],
    )
}

fn rounded(value: f64) -> String {
    if value < 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_under_ten_keep_one_decimal() {
        assert_eq!(rounded(1.24), "1.2");
        assert_eq!(rounded(9.96), "10.0");
        assert_eq!(rounded(12.6), "13");
    }
}
