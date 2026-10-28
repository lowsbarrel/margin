use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, Div, FontWeight, IntoElement, ParentElement, Pixels, SharedString, Styled, div, px,
};
use margin_i18n::t;
use margin_theme::{MarginTheme, TextScale};

pub(crate) const CONTROL_HEIGHT: Pixels = px(34.);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldError {
    Required,
    NotHttpUrl,
}

impl FieldError {
    const fn message_key(self) -> &'static str {
        match self {
            Self::Required => "settings_error_required",
            Self::NotHttpUrl => "settings_error_url",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InvalidForm;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Neutral,
    Positive,
    Negative,
}

pub(crate) fn required(value: &str) -> Option<FieldError> {
    value.trim().is_empty().then_some(FieldError::Required)
}

pub(crate) fn labeled(
    label: SharedString,
    control: impl IntoElement,
    error: Option<FieldError>,
    cx: &App,
) -> Div {
    let palette = MarginTheme::palette(cx);
    v_flex()
        .gap_1p5()
        .child(
            div()
                .text_ui_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.text_secondary)
                .child(label),
        )
        .child(control)
        .when_some(error, |this, error| {
            this.child(
                div()
                    .text_ui_xs()
                    .text_color(palette.text_negative)
                    .child(t(cx, error.message_key())),
            )
        })
}

pub(crate) fn field_hint(field: Div, hint: SharedString, cx: &App) -> Div {
    field.child(
        div()
            .text_ui_xs()
            .text_color(MarginTheme::palette(cx).text_tertiary)
            .child(hint),
    )
}

pub(crate) fn note(text: SharedString, tone: Tone, cx: &App) -> Div {
    let palette = MarginTheme::palette(cx);
    let color = match tone {
        Tone::Neutral => palette.text_tertiary,
        Tone::Positive => palette.text_positive,
        Tone::Negative => palette.text_negative,
    };
    div().text_ui_xs().italic().text_color(color).child(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_values_are_required() {
        assert_eq!(required("  "), Some(FieldError::Required));
        assert_eq!(required("x"), None);
    }
}
