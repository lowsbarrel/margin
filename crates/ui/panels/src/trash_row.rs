use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable, Icon, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Div, FontWeight, HighlightStyle, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, StyledText, div,
};
use margin_i18n::{t, t_count, t_with};
use margin_kernel::{RelativePath, UnixMillis};
use margin_theme::{MarginTheme, Palette, TextScale};
use margin_trash::TrashItem;
use margin_workbench::{OpenRequest, WorkbenchEvent};

use crate::panel_header::icon_button;
use crate::trash_filter::highlight_ranges;
use crate::trash_view::{TrashEventHandler, TrashView};
use crate::when::{self, Elapsed};

const PURGE_HINT_DAYS: u64 = 7;
const ROW_GROUP: &str = "trash-item";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowSelection {
    Selected,
    Unselected,
}

pub(crate) fn trash_row(
    index: usize,
    item: &TrashItem,
    terms: &[String],
    now: UnixMillis,
    selection: RowSelection,
    busy: bool,
    cx: &Context<'_, TrashView>,
) -> AnyElement {
    let palette = MarginTheme::palette(cx).clone();
    let selected = selection == RowSelection::Selected;
    let hover = palette.surface(3);
    let icon = if item.is_dir {
        IconName::Folder
    } else {
        IconName::FileText
    };
    h_flex()
        .id(("trash-item", index))
        .group(ROW_GROUP)
        .w_full()
        .gap_2p5()
        .py_1p5()
        .pl_2p5()
        .pr_2()
        .rounded_xl()
        .map(|row| {
            if selected {
                row.bg(palette.brand(16.0))
            } else {
                row.hover(move |style| style.bg(hover))
            }
        })
        .child(
            Icon::new(icon)
                .flex_shrink_0()
                .text_color(palette.text_tertiary),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(row_title(index, item, terms, &palette, cx))
                .child(row_details(item, terms, now, &palette, cx)),
        )
        .child(
            h_flex()
                .flex_shrink_0()
                .gap_1()
                .when(!selected, |actions| {
                    actions
                        .opacity(0.)
                        .group_hover(ROW_GROUP, |style| style.opacity(1.))
                })
                .child(
                    Button::new(("trash-restore", index))
                        .small()
                        .icon(IconName::RotateCcw)
                        .label(t(cx, "trash_restore"))
                        .disabled(busy)
                        .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            view.restore(index, window, cx);
                        })),
                )
                .child(
                    icon_button(
                        ("trash-delete", index),
                        IconName::Trash,
                        t(cx, "trash_delete_forever"),
                    )
                    .disabled(busy)
                    .on_click(cx.listener(
                        move |view, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            view.confirm_delete(index, window, cx);
                        },
                    )),
                ),
        )
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
            view.select(index, cx);
        }))
        .into_any_element()
}

fn row_title(index: usize, item: &TrashItem, terms: &[String], palette: &Palette, cx: &App) -> Div {
    h_flex()
        .gap_1p5()
        .min_w_0()
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_ui_sm()
                .text_color(palette.text_primary)
                .child(marked(&item.name, terms, palette)),
        )
        .when(item.has_history, |title| {
            let label = t(cx, "trash_includes_history");
            title.child(
                h_flex()
                    .id(("trash-history", index))
                    .size_4()
                    .flex_shrink_0()
                    .justify_center()
                    .rounded_sm()
                    .bg(palette.surface(2))
                    .text_color(palette.text_tertiary)
                    .child(Icon::new(IconName::RotateCcwClock).size_2p5())
                    .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx)),
            )
        })
}

fn row_details(
    item: &TrashItem,
    terms: &[String],
    now: UnixMillis,
    palette: &Palette,
    cx: &App,
) -> Div {
    let folder = item
        .path
        .parent()
        .filter(|parent| !parent.is_root())
        .map_or_else(
            || t(cx, "trash_vault_root").into_any_element(),
            |parent| marked(parent.as_str(), terms, palette).into_any_element(),
        );
    let purge_days = when::days_until_purge(item.deleted_at, now);
    h_flex()
        .gap_1p5()
        .min_w_0()
        .text_ui_xs()
        .text_color(palette.text_tertiary)
        .child(div().min_w_0().truncate().child(folder))
        .child(div().flex_shrink_0().child("\u{b7}"))
        .child(
            div()
                .flex_shrink_0()
                .whitespace_nowrap()
                .child(Elapsed::between(item.deleted_at, now).label(cx)),
        )
        .when(purge_days <= PURGE_HINT_DAYS, |details| {
            details.child(
                div()
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .text_color(palette.text_warning)
                    .child(when::purge_label(purge_days, cx)),
            )
        })
}

fn marked(text: &str, terms: &[String], palette: &Palette) -> StyledText {
    let style = HighlightStyle {
        color: Some(palette.text_primary),
        background_color: Some(palette.brand(32.0)),
        font_weight: Some(FontWeight::MEDIUM),
        ..HighlightStyle::default()
    };
    StyledText::new(SharedString::from(text.to_owned())).with_highlights(
        highlight_ranges(text, terms)
            .into_iter()
            .map(|range| (range, style)),
    )
}

pub(crate) fn restored_notification(
    name: &str,
    openable: Option<RelativePath>,
    on_event: &TrashEventHandler,
    cx: &App,
) -> Notification {
    let notification = Notification::success(t_with(cx, "trash_restored", &[("name", name)]));
    let Some(path) = openable else {
        return notification;
    };
    let on_event = Rc::clone(on_event);
    let label = t(cx, "trash_open");
    notification.action(move |_, _, _| {
        let on_event = Rc::clone(&on_event);
        let path = path.clone();
        Button::new("trash-open-restored")
            .small()
            .label(label.clone())
            .on_click(move |_, window, cx| {
                on_event(
                    WorkbenchEvent::Open(OpenRequest::new(path.clone())),
                    window,
                    cx,
                );
            })
    })
}

pub(crate) fn group_header(label: &SharedString, cx: &App) -> Div {
    div()
        .px_2p5()
        .pb_1()
        .text_ui_xs()
        .font_semibold()
        .text_color(MarginTheme::palette(cx).text_tertiary)
        .child(label.to_uppercase())
}

pub(crate) fn trash_footer(
    total: usize,
    shown: Option<usize>,
    busy: bool,
    cx: &Context<'_, TrashView>,
) -> Div {
    let palette = MarginTheme::palette(cx);
    let count = match shown {
        Some(shown) => {
            let shown = shown.to_string();
            let total = total.to_string();
            t_with(
                cx,
                "trash_items_filtered",
                &[("shown", shown.as_str()), ("total", total.as_str())],
            )
        }
        None => t_count(
            cx,
            "trash_items",
            u64::try_from(total).unwrap_or(u64::MAX),
            &[],
        ),
    };
    h_flex()
        .justify_between()
        .gap_3()
        .pt_3()
        .border_t_1()
        .border_color(palette.border_secondary)
        .child(
            div()
                .text_ui_xs()
                .text_color(palette.text_tertiary)
                .child(count),
        )
        .child(
            Button::new("trash-empty")
                .danger()
                .small()
                .icon(IconName::Trash)
                .label(t(cx, "trash_empty_trash"))
                .disabled(busy)
                .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                    TrashView::confirm_empty(window, cx);
                })),
        )
}
