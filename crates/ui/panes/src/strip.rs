use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::{
    AnyElement, App, Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Rems, SharedString, StatefulInteractiveElement, Styled, Window, div, px, rems,
};
use margin_i18n::t;
use margin_theme::{MarginTheme, Palette, TextScale};
use margin_workbench::{DraggedFiles, file_title};

use crate::drag::DraggedTab;
use crate::layout::{PaneId, Tab};
use crate::panes::{PaneItem, Panes};

pub(crate) const STRIP_HEIGHT: Rems = Rems(2.5);
const TAB_MIN_WIDTH: Rems = Rems(7.5);
const TITLE_MAX_WIDTH: Rems = Rems(10.0);
const CLOSE_TAB_SIZE: f32 = 18.0;
const CLOSE_PANE_SIZE: f32 = 26.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum GapEdge {
    Before,
    After,
}

struct TabState {
    pane: usize,
    pane_id: PaneId,
    index: usize,
    active: bool,
    focused_pane: bool,
    gap: Option<GapEdge>,
    draggable: bool,
}

pub(crate) fn render_strip<T: PaneItem>(
    this: &Panes<T>,
    pane: usize,
    cx: &Context<'_, Panes<T>>,
) -> AnyElement {
    let palette = MarginTheme::palette(cx).clone();
    let entry = &this.layout.panes()[pane];
    let pane_id = entry.id;
    let gap = this.strip_gap(pane);
    let single_tab = this.layout.panes().len() == 1 && entry.tabs.len() == 1;
    let scroll = this.scrolls.get(&pane_id).cloned().unwrap_or_default();
    let tab_count = entry.tabs.len();
    let tabs: Vec<AnyElement> = entry
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let state = TabState {
                pane,
                pane_id,
                index,
                active: entry.active == Some(index),
                focused_pane: this.layout.active_pane() == pane,
                gap: if gap == Some(index) {
                    Some(GapEdge::Before)
                } else if index + 1 == tab_count && gap == Some(tab_count) {
                    Some(GapEdge::After)
                } else {
                    None
                },
                draggable: !single_tab,
            };
            render_tab(tab, &state, &palette, cx)
        })
        .collect();
    let row = div()
        .id(("pane-tabs", pane_id.value()))
        .flex()
        .flex_1()
        .h_full()
        .overflow_x_scroll()
        .track_scroll(&scroll)
        .children(tabs);
    h_flex()
        .id(("pane-strip", pane_id.value()))
        .relative()
        .flex_shrink_0()
        .h(STRIP_HEIGHT)
        .min_h(STRIP_HEIGHT)
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(palette.border_secondary)
        .bg(if gap.is_some() {
            palette.brand(8.0)
        } else {
            palette.surface(1)
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, window, cx| this.focus_pane(pane, window, cx)),
        )
        .on_drop(cx.listener(|this, payload: &DraggedTab, window, cx| {
            this.drop_tab_on_strip(payload, window, cx);
        }))
        .on_drop(cx.listener(|this, payload: &DraggedFiles, window, cx| {
            this.drop_files_on_strip(payload, window, cx);
        }))
        .child(row)
        .children((this.layout.panes().len() > 1).then(|| close_pane(pane, &palette, cx)))
        .into_any_element()
}

fn render_tab<T: PaneItem>(
    tab: &Tab<T>,
    state: &TabState,
    palette: &Palette,
    cx: &Context<'_, Panes<T>>,
) -> AnyElement {
    let handle = tab.item.handle();
    let id = handle.id();
    let path = handle.path(cx);
    let title = SharedString::from(file_title(&path).to_owned());
    let (pane, index) = (state.pane, state.index);
    let mut element = h_flex()
        .id(("pane-tab", id.as_u64()))
        .relative()
        .h(STRIP_HEIGHT)
        .flex_shrink_1()
        .cursor_pointer()
        .items_center()
        .gap_1p5()
        .border_r_1()
        .border_color(palette.border_secondary)
        .px_2p5()
        .text_ui_xs()
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap();
    if !tab.pinned {
        element = element.min_w(TAB_MIN_WIDTH);
    }
    element = if state.active {
        let underline = if state.focused_pane {
            palette.bg_brand
        } else {
            palette.text_secondary
        };
        element
            .bg(palette.surface(0))
            .text_color(palette.text_primary)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(rems(0.125))
                    .bg(underline),
            )
    } else {
        let hover_text = palette.text_secondary;
        let hover_fill = palette.surface(3);
        element
            .text_color(palette.text_tertiary)
            .hover(move |style| style.bg(hover_fill).text_color(hover_text))
    };
    match state.gap {
        Some(GapEdge::Before) => element = element.child(gap_marker(palette).left(px(-1.0))),
        Some(GapEdge::After) => element = element.child(gap_marker(palette).right(px(-1.0))),
        None => {}
    }
    if tab.pinned {
        element = element.child(
            Icon::new(IconName::Pin)
                .size(px(11.0))
                .flex_shrink_0()
                .text_color(palette.text_brand),
        );
    } else {
        element = element.child(
            div()
                .min_w_0()
                .max_w(TITLE_MAX_WIDTH)
                .flex_shrink_1()
                .truncate()
                .child(title.clone()),
        );
    }
    if state.draggable {
        let payload = DraggedTab {
            pane: state.pane_id,
            item: id,
            pinned: tab.pinned,
            label: title.clone(),
        };
        element = element.on_drag(payload, |payload, offset, _, cx| {
            payload.preview(offset, cx)
        });
    }
    let entity = cx.entity();
    let pinned = tab.pinned;
    element
        .child(close_tab(pane, index, palette, cx))
        .tooltip(move |window, cx| Tooltip::new(title.clone()).build(window, cx))
        .on_click(cx.listener(move |this, _, window, cx| this.activate(pane, index, window, cx)))
        .on_mouse_up(
            MouseButton::Middle,
            cx.listener(move |this, _, window, cx| this.close(pane, index, window, cx)),
        )
        .context_menu(move |menu, _, cx| tab_menu(menu, &entity, pane, index, pinned, cx))
        .into_any_element()
}

fn gap_marker(palette: &Palette) -> gpui_kit::Div {
    div()
        .absolute()
        .top_1()
        .bottom_1()
        .w(rems(0.125))
        .rounded_full()
        .bg(palette.bg_brand)
}

fn close_tab<T: PaneItem>(
    pane: usize,
    index: usize,
    palette: &Palette,
    cx: &Context<'_, Panes<T>>,
) -> AnyElement {
    let hover_fill = palette.surface(3);
    let hover_text = palette.text_primary;
    div()
        .id(("pane-tab-close", index))
        .flex()
        .flex_shrink_0()
        .size(px(CLOSE_TAB_SIZE))
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .cursor_pointer()
        .text_color(palette.text_tertiary)
        .hover(move |style| style.bg(hover_fill).text_color(hover_text))
        .tooltip(|window, cx| Tooltip::new(t(cx, "tab_close_label")).build(window, cx))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _, window, cx| {
            cx.stop_propagation();
            this.close(pane, index, window, cx);
        }))
        .child(Icon::new(IconName::X).size(px(12.0)))
        .into_any_element()
}

fn close_pane<T: PaneItem>(
    pane: usize,
    palette: &Palette,
    cx: &Context<'_, Panes<T>>,
) -> AnyElement {
    let hover_fill = palette.surface(3);
    let hover_text = palette.text_primary;
    h_flex()
        .h_full()
        .flex_shrink_0()
        .items_center()
        .gap_0p5()
        .px_1p5()
        .border_l_1()
        .border_color(palette.border_secondary)
        .child(
            div()
                .id(("pane-close", pane))
                .flex()
                .size(px(CLOSE_PANE_SIZE))
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .cursor_pointer()
                .text_color(palette.text_tertiary)
                .hover(move |style| style.bg(hover_fill).text_color(hover_text))
                .tooltip(|window, cx| Tooltip::new(t(cx, "pane_close")).build(window, cx))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.close_pane(pane, window, cx);
                }))
                .child(Icon::new(IconName::X).size(px(14.0))),
        )
        .into_any_element()
}

type TabAction<T> = fn(&mut Panes<T>, usize, usize, &mut Window, &mut Context<'_, Panes<T>>);

fn tab_menu<T: PaneItem>(
    menu: PopupMenu,
    panes: &Entity<Panes<T>>,
    pane: usize,
    index: usize,
    pinned: bool,
    cx: &App,
) -> PopupMenu {
    let state = panes.read(cx);
    let tab_count = state
        .layout
        .panes()
        .get(pane)
        .map_or(0, |entry| entry.tabs.len());
    let can_reopen = state.can_reopen();
    let pin_label = if pinned { "tab_unpin" } else { "tab_pin" };
    let item = |key: &'static str, run: TabAction<T>| {
        let panes = panes.clone();
        PopupMenuItem::new(t(cx, key)).on_click(move |_, window, cx| {
            panes.update(cx, |panes, cx| run(panes, pane, index, window, cx));
        })
    };
    menu.item(item(pin_label, |panes, pane, index, window, cx| {
        panes.toggle_pin(pane, index, window, cx);
    }))
    .item(item("tab_close", |panes, pane, index, window, cx| {
        panes.close(pane, index, window, cx);
    }))
    .item(
        item("tab_close_others", |panes, pane, index, window, cx| {
            panes.close_others(pane, index, window, cx);
        })
        .disabled(tab_count <= 1),
    )
    .item(item("tab_close_all", |panes, pane, _, window, cx| {
        panes.close_all(pane, window, cx);
    }))
    .item(
        item("tab_reopen_closed", |panes, _, _, _, cx| {
            panes.reopen_closed(cx);
        })
        .disabled(!can_reopen),
    )
}
