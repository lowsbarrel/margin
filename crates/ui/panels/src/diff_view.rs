use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, IntoElement, ListHorizontalSizingBehavior, ParentElement, SharedString,
    Styled, div, relative, rems, uniform_list,
};
use margin_i18n::t;
use margin_theme::{MONO_FONT_FAMILY, MarginTheme, TextScale};

use crate::line_diff::{DiffLine, LineDiff, LineKind};

const LINE_HEIGHT_REMS: f32 = 18.0 / 14.0;
const MAX_BODY_HEIGHT_REMS: f32 = 24.5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PreviewDiff {
    Loading,
    Ready(Rc<LineDiff>),
    TooLarge,
}

pub(crate) fn diff_view(preview: &PreviewDiff, cx: &App) -> Option<AnyElement> {
    let palette = MarginTheme::palette(cx);
    let (counts, body) = match preview {
        PreviewDiff::Loading => return None,
        PreviewDiff::TooLarge => (
            None,
            div()
                .px_2()
                .py_1()
                .text_ui_xs()
                .text_color(palette.text_tertiary)
                .child(t(cx, "history_diff_too_large"))
                .into_any_element(),
        ),
        PreviewDiff::Ready(diff) => (
            Some(
                h_flex()
                    .gap_1p5()
                    .flex_shrink_0()
                    .child(
                        div()
                            .text_color(palette.text_positive)
                            .child(format!("+{}", diff.added())),
                    )
                    .child(
                        div()
                            .text_color(palette.text_negative)
                            .child(format!("\u{2212}{}", diff.removed())),
                    ),
            ),
            lines_view(diff),
        ),
    };
    let header = h_flex()
        .gap_2()
        .justify_between()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(palette.border_secondary)
        .text_ui_xs()
        .text_color(palette.text_tertiary)
        .child(t(cx, "history_diff_current"))
        .children(counts);
    Some(
        v_flex()
            .mx_2()
            .my_1()
            .rounded_lg()
            .overflow_hidden()
            .border_1()
            .border_color(palette.border_secondary)
            .bg(palette.surface(0))
            .child(header)
            .child(body)
            .into_any_element(),
    )
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a diff never holds enough lines to lose precision as a height"
)]
fn lines_view(diff: &Rc<LineDiff>) -> AnyElement {
    let count = diff.lines().len();
    let height = (count as f32 * LINE_HEIGHT_REMS).min(MAX_BODY_HEIGHT_REMS);
    let widest = diff
        .lines()
        .iter()
        .enumerate()
        .max_by_key(|(_, line)| line.text().chars().count())
        .map(|(index, _)| index);
    let lines = Rc::clone(diff);
    div()
        .py_1()
        .child(
            uniform_list("history-diff", count, move |range, _, cx| {
                lines
                    .lines()
                    .get(range)
                    .unwrap_or_default()
                    .iter()
                    .map(|line| line_view(line, cx))
                    .collect()
            })
            .with_width_from_item(widest)
            .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
            .h(rems(height))
            .font_family(MONO_FONT_FAMILY)
            .text_ui_xs()
            .line_height(relative(1.5)),
        )
        .into_any_element()
}

fn line_view(line: &DiffLine, cx: &App) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let number = |value: Option<usize>| {
        div()
            .w_9()
            .flex_shrink_0()
            .pr_1p5()
            .text_right()
            .text_color(palette.text_tertiary)
            .child(value.map(|value| value.to_string()).unwrap_or_default())
    };
    let (marker, color, fill) = match line.kind() {
        LineKind::Same => ("  ", palette.text_secondary, None),
        LineKind::Added => ("+ ", palette.text_positive, Some(palette.surface(2))),
        LineKind::Removed => ("- ", palette.text_negative, Some(palette.surface(2))),
    };
    let mut row = h_flex()
        .h(rems(LINE_HEIGHT_REMS))
        .w_full()
        .whitespace_nowrap()
        .text_color(color)
        .child(number(line.before()))
        .child(number(line.after()))
        .child(
            div()
                .flex_shrink_0()
                .child(SharedString::from(format!("{marker}{}", line.text()))),
        );
    if let Some(fill) = fill {
        row = row.bg(fill);
    }
    row.into_any_element()
}
