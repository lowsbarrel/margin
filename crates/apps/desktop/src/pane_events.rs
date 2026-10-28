use gpui_kit::{Context, Entity, Window};
use margin_kernel::RelativePath;
use margin_panes::{Panes, PanesEvent, Side};
use margin_workbench::{OpenRequest, OpenTarget};

use crate::tab_view::TabView;
use crate::workspace::Workspace;

pub(crate) fn on_event(
    workspace: &mut Workspace,
    _: &Entity<Panes<TabView>>,
    event: &PanesEvent,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    match event {
        PanesEvent::Changed => {
            track_tabs(workspace, window, cx);
            workspace.active_changed(cx);
            workspace.schedule_save(window, cx);
        }
        PanesEvent::Open { path, pane, at } => {
            open_in(workspace, path, Some(*pane), *at, window, cx);
        }
        PanesEvent::OpenSplit { path, pane, side } => {
            split(workspace, path, *pane, *side, window, cx);
        }
    }
}

pub(crate) fn show(
    workspace: &Workspace,
    request: &OpenRequest,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) -> TabView {
    match request.target {
        OpenTarget::ActivePane | OpenTarget::NewTab => {
            open_in(workspace, &request.path, None, None, window, cx)
        }
        OpenTarget::SplitRight => {
            let pane = workspace.panes.read(cx).layout().active_pane();
            split(workspace, &request.path, pane, Side::Right, window, cx)
        }
    }
}

fn track_tabs(workspace: &mut Workspace, window: &Window, cx: &mut Context<'_, Workspace>) {
    let live: Vec<TabView> = workspace.panes.read(cx).layout().items().cloned().collect();
    workspace
        .tab_events
        .retain(|id, _| live.iter().any(|view| view.entity_id() == *id));
    for view in live {
        workspace
            .tab_events
            .entry(view.entity_id())
            .or_insert_with(|| view.subscribe(window, cx));
    }
}

fn open_in(
    workspace: &Workspace,
    path: &RelativePath,
    pane: Option<usize>,
    at: Option<usize>,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) -> TabView {
    workspace.panes.update(cx, |panes, cx| {
        let view = panes.open(
            pane,
            at,
            path,
            |window, cx| TabView::open(path.clone(), window, cx),
            window,
            cx,
        );
        panes.focus_active(window, cx);
        view
    })
}

fn split(
    workspace: &Workspace,
    path: &RelativePath,
    pane: usize,
    side: Side,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) -> TabView {
    let view = TabView::open(path.clone(), window, cx);
    workspace.panes.update(cx, |panes, cx| {
        panes.open_split(pane, side, view.clone(), window, cx);
        panes.focus_active(window, cx);
    });
    view
}
