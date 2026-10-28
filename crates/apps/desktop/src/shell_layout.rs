use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Div, DragMoveEvent, EmptyView, InteractiveElement,
    IntoElement, ParentElement, Rems, Render, Stateful, StatefulInteractiveElement, Styled, Window,
    div, px, rems,
};
use margin_i18n::t;
use margin_theme::{MarginTheme, Palette, TextScale};
use margin_workbench::WORKSPACE_CONTEXT;

use crate::workspace::{RightPanel, Workspace};
use crate::{status_bar, workspace_actions};

const SIDEBAR_MIN: f32 = 180.0;
const SIDEBAR_MAX: f32 = 480.0;
const RIGHT_PANEL_WIDTH: Rems = rems(18.75);
const TERMINAL_MIN: f32 = 120.0;
const TERMINAL_MAX_SHARE: f32 = 0.7;
const HANDLE_THICKNESS: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShellEdge {
    Sidebar,
    Terminal,
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let status = status_bar::render(self, cx);
        let spotlight = self
            .spotlight
            .read(cx)
            .is_open()
            .then(|| self.spotlight.clone());
        let palette = MarginTheme::palette(cx).clone();
        let title = h_flex()
            .w_full()
            .justify_center()
            .text_ui_sm()
            .text_color(palette.text_tertiary)
            .child(t(cx, "app_display_name"));
        let root = v_flex()
            .size_full()
            .key_context(WORKSPACE_CONTEXT)
            .track_focus(&self.focus)
            .bg(palette.surface(0))
            .child(TitleBar::new().child(title))
            .child(shell(self, window, cx))
            .child(status)
            .children(spotlight.map(|spotlight| div().absolute().inset_0().child(spotlight)));
        workspace_actions::register(root, cx)
    }
}

fn shell(workspace: &mut Workspace, window: &Window, cx: &Context<'_, Workspace>) -> AnyElement {
    if !cx.has_active_drag() {
        workspace.resizing = None;
    }
    let palette = MarginTheme::palette(cx).clone();
    let layout = workspace.layout;
    let terminal_max = f32::from(window.viewport_size().height * TERMINAL_MAX_SHARE);
    let mut center = v_flex()
        .id("shell-center")
        .flex_1()
        .min_w_0()
        .h_full()
        .child(div().flex_1().min_h_0().child(workspace.panes.clone()))
        .on_drag_move(
            cx.listener(move |this, event: &DragMoveEvent<ShellEdge>, window, cx| {
                if *event.drag(cx) == ShellEdge::Terminal {
                    let height = f32::from(event.bounds.bottom() - event.event.position.y);
                    this.layout.terminal_height =
                        height.min(terminal_max).max(TERMINAL_MIN).round();
                    resized(this, ShellEdge::Terminal, window, cx);
                }
            }),
        );
    if layout.terminal_open {
        center = center.child(
            div()
                .relative()
                .flex_shrink_0()
                .h(px(layout
                    .terminal_height
                    .min(terminal_max)
                    .max(TERMINAL_MIN)))
                .border_t_1()
                .border_color(palette.border_secondary)
                .bg(palette.surface(1))
                .child(workspace.terminal.clone())
                .child(
                    handle(ShellEdge::Terminal, workspace.resizing, &palette)
                        .left_0()
                        .right_0()
                        .top(px(-HANDLE_THICKNESS / 2.0))
                        .h(px(HANDLE_THICKNESS))
                        .cursor_row_resize(),
                ),
        );
    }
    let mut row = h_flex()
        .id("shell-row")
        .flex_1()
        .min_h_0()
        .w_full()
        .on_drag_move(
            cx.listener(|this, event: &DragMoveEvent<ShellEdge>, window, cx| {
                if *event.drag(cx) == ShellEdge::Sidebar {
                    let width = f32::from(event.event.position.x - event.bounds.left());
                    this.layout.sidebar_width = width.clamp(SIDEBAR_MIN, SIDEBAR_MAX).round();
                    resized(this, ShellEdge::Sidebar, window, cx);
                }
            }),
        );
    if layout.sidebar_open {
        row = row.child(sidebar(workspace, &palette));
    }
    row = row.child(center);
    if let Some(panel) = right_panel(workspace, cx) {
        row = row.child(
            div()
                .flex_shrink_0()
                .w(RIGHT_PANEL_WIDTH)
                .h_full()
                .border_l_1()
                .border_color(palette.border_secondary)
                .bg(palette.surface(1))
                .child(panel),
        );
    }
    if workspace.resizing == Some(ShellEdge::Sidebar) {
        row = row.cursor_col_resize();
    } else if workspace.resizing == Some(ShellEdge::Terminal) {
        row = row.cursor_row_resize();
    }
    row.into_any_element()
}

fn sidebar(workspace: &Workspace, palette: &Palette) -> AnyElement {
    div()
        .relative()
        .flex_shrink_0()
        .w(px(workspace
            .layout
            .sidebar_width
            .clamp(SIDEBAR_MIN, SIDEBAR_MAX)))
        .h_full()
        .overflow_hidden()
        .border_r_1()
        .border_color(palette.border_secondary)
        .child(workspace.file_tree.clone())
        .child(
            handle(ShellEdge::Sidebar, workspace.resizing, palette)
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(HANDLE_THICKNESS))
                .cursor_col_resize(),
        )
        .into_any_element()
}

fn right_panel(workspace: &Workspace, cx: &App) -> Option<AnyElement> {
    workspace.active_tab(cx)?.note()?;
    Some(match workspace.right_panel? {
        RightPanel::Backlinks => workspace.backlinks.clone().into_any_element(),
        RightPanel::History => workspace.history.clone().into_any_element(),
    })
}

fn handle(edge: ShellEdge, resizing: Option<ShellEdge>, palette: &Palette) -> Stateful<Div> {
    let hover = palette.brand(25.0);
    let handle = div()
        .id(match edge {
            ShellEdge::Sidebar => "sidebar-resize",
            ShellEdge::Terminal => "terminal-resize",
        })
        .absolute()
        .occlude()
        .on_drag(edge, |_, _, _, cx| cx.new(|_| EmptyView));
    if resizing == Some(edge) {
        handle.bg(palette.brand(50.0))
    } else {
        handle.hover(move |style| style.bg(hover))
    }
}

fn resized(
    workspace: &mut Workspace,
    edge: ShellEdge,
    window: &Window,
    cx: &mut Context<'_, Workspace>,
) {
    workspace.resizing = Some(edge);
    workspace.schedule_save(window, cx);
    cx.notify();
}
