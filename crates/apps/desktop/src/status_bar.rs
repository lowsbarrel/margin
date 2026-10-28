use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::{
    Action, AnyElement, AnyView, App, Div, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div,
};
use margin_editor::{NoteEditor, ViewMode};
use margin_i18n::{t, t_with};
use margin_theme::{Appearance, MarginTheme, TextScale, css_px};
use margin_workbench::{
    LockVault, OpenSettings, OpenTrash, ToggleBacklinks, ToggleHistory, ToggleSidebar,
    ToggleTerminal, ToggleViewMode, WORKSPACE_CONTEXT,
};

use crate::workspace::{RightPanel, Workspace};

pub(crate) fn render(workspace: &Workspace, cx: &App) -> AnyElement {
    let editor = workspace
        .active_tab(cx)
        .and_then(|view| view.note().cloned());
    let mut left = h_flex()
        .gap_1p5()
        .items_center()
        .min_w_0()
        .child(sidebar_toggle(workspace, cx));
    let mut right = h_flex()
        .gap_1p5()
        .items_center()
        .flex_shrink_0()
        .child(workspace.sync_status.clone());
    if let Some(editor) = editor {
        left = left.children(note_position(&editor, cx));
        right = right.children(note_controls(workspace, &editor, cx));
    }
    let palette = MarginTheme::palette(cx);
    h_flex()
        .h_9()
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .gap_3()
        .pl_4()
        .pr_2()
        .border_t_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface(1))
        .text_ui_xs()
        .whitespace_nowrap()
        .text_color(palette.text_tertiary)
        .child(left)
        .child(app_controls(workspace, right, cx))
        .into_any_element()
}

fn sidebar_toggle(workspace: &Workspace, cx: &App) -> Stateful<Div> {
    let open = workspace.layout.sidebar_open;
    let label = if open {
        "sidebar_close_panel"
    } else {
        "sidebar_open_panel"
    };
    icon_button(
        "status-sidebar",
        IconName::PanelLeft,
        t(cx, label),
        &ToggleSidebar,
        open,
        cx,
    )
}

fn note_position(editor: &Entity<NoteEditor>, cx: &App) -> [AnyElement; 6] {
    let (line, column) = editor.read(cx).cursor_line_column();
    let line = line.to_string();
    let column = column.to_string();
    let position = t_with(
        cx,
        "statusbar_line_col",
        &[("line", &line), ("col", &column)],
    );
    [
        separator(cx),
        div().truncate().child(position).into_any_element(),
        separator(cx),
        t(cx, "statusbar_markdown").into_any_element(),
        separator(cx),
        t(cx, "statusbar_encoding").into_any_element(),
    ]
}

fn note_controls(
    workspace: &Workspace,
    editor: &Entity<NoteEditor>,
    cx: &App,
) -> Vec<Stateful<Div>> {
    let raw = editor.read(cx).view_mode() == ViewMode::Raw;
    let view_label = if raw {
        "statusbar_view_rich"
    } else {
        "statusbar_view_source"
    };
    let exported = editor.clone();
    vec![
        plain_button("status-export", IconName::FileDown, false, cx)
            .tooltip(tooltip(t(cx, "statusbar_export_pdf")))
            .on_click(move |_, window, cx| {
                exported.update(cx, |editor, cx| editor.export_html(window, cx));
            }),
        icon_button(
            "status-backlinks",
            IconName::Link2,
            t(cx, "statusbar_backlinks"),
            &ToggleBacklinks,
            workspace.right_panel == Some(RightPanel::Backlinks),
            cx,
        ),
        icon_button(
            "status-view-mode",
            IconName::FileCode,
            t(cx, view_label),
            &ToggleViewMode,
            raw,
            cx,
        ),
        icon_button(
            "status-history",
            IconName::Clock,
            t(cx, "statusbar_history"),
            &ToggleHistory,
            workspace.right_panel == Some(RightPanel::History),
            cx,
        ),
    ]
}

fn app_controls(workspace: &Workspace, row: Div, cx: &App) -> Div {
    let dark = MarginTheme::palette(cx).dark;
    row.child(icon_button(
        "status-terminal",
        IconName::SquareTerminal,
        t(cx, "statusbar_terminal"),
        &ToggleTerminal,
        workspace.layout.terminal_open,
        cx,
    ))
    .child(icon_button(
        "status-trash",
        IconName::Trash,
        t(cx, "statusbar_trash"),
        &OpenTrash,
        false,
        cx,
    ))
    .child(icon_button(
        "status-settings",
        IconName::Settings,
        t(cx, "statusbar_settings"),
        &OpenSettings,
        false,
        cx,
    ))
    .child(
        plain_button(
            "status-theme",
            if dark { IconName::Sun } else { IconName::Moon },
            false,
            cx,
        )
        .tooltip(tooltip(t(cx, "statusbar_toggle_theme")))
        .on_click(move |_, _, cx| {
            let next = if dark {
                Appearance::Light
            } else {
                Appearance::Dark
            };
            MarginTheme::set_appearance(next, cx);
        }),
    )
    .child(vault_pill(workspace, cx))
    .child(icon_button(
        "status-lock",
        IconName::LogOut,
        t(cx, "statusbar_lock_vault"),
        &LockVault,
        false,
        cx,
    ))
}

fn icon_button(
    id: &'static str,
    icon: IconName,
    label: SharedString,
    action: &dyn Action,
    active: bool,
    cx: &App,
) -> Stateful<Div> {
    let dispatched = action.boxed_clone();
    let described = action.boxed_clone();
    plain_button(id, icon, active, cx)
        .tooltip(move |window, cx| {
            Tooltip::new(label.clone())
                .action(described.as_ref(), Some(WORKSPACE_CONTEXT))
                .build(window, cx)
        })
        .on_click(move |_, window, cx| window.dispatch_action(dispatched.boxed_clone(), cx))
}

fn plain_button(id: &'static str, icon: IconName, active: bool, cx: &App) -> Stateful<Div> {
    let palette = MarginTheme::palette(cx);
    let hover_bg = palette.surface(2);
    let hover_text = palette.text_primary;
    let button = h_flex()
        .id(id)
        .flex_shrink_0()
        .size(css_px(26.0))
        .justify_center()
        .rounded(css_px(8.0))
        .cursor_pointer()
        .child(Icon::new(icon).size(css_px(14.0)));
    if active {
        button
            .bg(palette.brand(12.0))
            .text_color(palette.text_brand)
    } else {
        button.hover(move |style| style.bg(hover_bg).text_color(hover_text))
    }
}

fn tooltip(label: SharedString) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |window, cx| Tooltip::new(label.clone()).build(window, cx)
}

fn vault_pill(workspace: &Workspace, cx: &App) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let hover_border = palette.border_strong;
    let hover_bg = palette.surface(2);
    let hover_text = palette.text_primary;
    h_flex()
        .id("status-switch-vault")
        .h_6()
        .max_w_40()
        .px_2()
        .gap(css_px(5.0))
        .items_center()
        .rounded_full()
        .border_1()
        .border_color(palette.border_secondary)
        .text_color(palette.text_secondary)
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |style| {
            style
                .border_color(hover_border)
                .bg(hover_bg)
                .text_color(hover_text)
        })
        .child(Icon::new(IconName::ArrowLeftRight).size(css_px(12.0)))
        .child(
            div()
                .min_w_0()
                .truncate()
                .child(SharedString::from(workspace.services.name.clone())),
        )
        .tooltip(tooltip(t(cx, "statusbar_switch_vault")))
        .on_click(|_, window, cx| window.dispatch_action(Box::new(LockVault), cx))
        .into_any_element()
}

fn separator(cx: &App) -> AnyElement {
    div()
        .text_color(MarginTheme::palette(cx).border_strong)
        .child(SharedString::new_static("·"))
        .into_any_element()
}
