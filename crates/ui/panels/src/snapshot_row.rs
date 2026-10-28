use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, ClickEvent, Context, Focusable, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div,
};
use margin_history::Snapshot;
use margin_i18n::t;
use margin_kernel::UnixMillis;
use margin_theme::{MarginTheme, TextScale};

use crate::diff_view::{PreviewDiff, diff_view};
use crate::history_panel::HistoryPanel;
use crate::panel_header::icon_button;
use crate::when;

const ROW_GROUP: &str = "history-snapshot";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowLook {
    Plain,
    Previewed,
    Selected { focused: bool },
}

pub(crate) fn stamp(snapshot: &Snapshot) -> UnixMillis {
    UnixMillis::from_seconds(snapshot.timestamp)
}

pub(crate) fn snapshot_row(
    index: usize,
    snapshot: &Snapshot,
    label: SharedString,
    preview: Option<&PreviewDiff>,
    look: RowLook,
    restoring: bool,
    cx: &Context<'_, HistoryPanel>,
) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let hover = palette.surface(3);
    let hover_text = palette.text_primary;
    let restore = icon_button(
        ("history-restore", index),
        IconName::RotateCcw,
        t(cx, "history_restore"),
    )
    .loading(restoring)
    .on_click(cx.listener(move |panel, _: &ClickEvent, window, cx| {
        cx.stop_propagation();
        panel.restore(index, window, cx);
    }));
    let delete = icon_button(
        ("history-delete", index),
        IconName::Trash,
        t(cx, "history_delete"),
    )
    .on_click(cx.listener(move |panel, _: &ClickEvent, window, cx| {
        cx.stop_propagation();
        panel.delete(index, window, cx);
    }));
    let actions_shown = look != RowLook::Plain || restoring;
    let row = h_flex()
        .id(("history-snapshot", index))
        .group(ROW_GROUP)
        .w_full()
        .justify_between()
        .p_2()
        .rounded_sm()
        .text_ui_sm()
        .map(|row| match look {
            RowLook::Plain => row
                .text_color(palette.text_secondary)
                .hover(move |style| style.bg(hover).text_color(hover_text)),
            RowLook::Previewed => row.bg(palette.surface(2)).text_color(palette.text_primary),
            RowLook::Selected { focused } => row
                .bg(palette.brand(if focused { 32.0 } else { 16.0 }))
                .text_color(palette.text_primary),
        })
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .child(Icon::new(IconName::Clock).xsmall())
                .child(div().whitespace_nowrap().child(label))
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_ui_xs()
                        .text_color(palette.text_tertiary)
                        .child(when::format_bytes(snapshot.size)),
                ),
        )
        .child(
            h_flex()
                .gap_0p5()
                .when(!actions_shown, |actions| {
                    actions
                        .opacity(0.)
                        .group_hover(ROW_GROUP, |style| style.opacity(1.))
                })
                .child(restore)
                .child(delete),
        )
        .on_click(cx.listener(move |panel, _: &ClickEvent, window, cx| {
            panel.focus_handle(cx).focus(window, cx);
            panel.click_snapshot(index, window, cx);
        }));
    v_flex()
        .w_full()
        .child(row)
        .children(preview.and_then(|preview| diff_view(preview, cx)))
        .into_any_element()
}
