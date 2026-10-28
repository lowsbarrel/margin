use gpui_kit::component::v_flex;
use gpui_kit::{AnyElement, App, IntoElement, ParentElement, Styled, div};
use margin_i18n::{t, t_with};
use margin_theme::{Palette, TextScale};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StatusChanged;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SessionStatus {
    Running,
    Exited(i32),
    Failed(String),
}

impl SessionStatus {
    pub(crate) fn overlay(&self, palette: &Palette, cx: &App) -> Option<AnyElement> {
        match self {
            Self::Running => None,
            Self::Exited(code) => {
                let code = code.to_string();
                Some(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .px_3()
                        .py_1()
                        .border_t_1()
                        .border_color(palette.border_secondary)
                        .bg(palette.surface(1))
                        .text_ui_xs()
                        .text_color(palette.text_tertiary)
                        .child(t_with(cx, "terminal_exited", &[("code", code.as_str())]))
                        .into_any_element(),
                )
            }
            Self::Failed(error) => Some(
                v_flex()
                    .absolute()
                    .inset_0()
                    .items_center()
                    .text_center()
                    .justify_center()
                    .gap_1()
                    .px_6()
                    .bg(palette.surface(0))
                    .text_ui_xs()
                    .child(
                        div()
                            .text_color(palette.text_negative)
                            .child(t(cx, "terminal_start_failed")),
                    )
                    .child(
                        div()
                            .text_color(palette.text_secondary)
                            .child(error.clone()),
                    )
                    .into_any_element(),
            ),
        }
    }
}
