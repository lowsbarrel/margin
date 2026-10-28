use gpui_kit::{Context, Div, InteractiveElement, Window};
use margin_editor::NoteEditor;
use margin_panes::Panes;
use margin_spotlight::SpotlightMode;
use margin_workbench::{
    CloseAllTabs, CloseOtherTabs, CloseTab, LockVault, NewNote, OpenSettings, OpenSpotlight,
    OpenTrash, ReopenClosedTab, SearchVault, SyncNow, ToggleBacklinks, ToggleHistory, TogglePinTab,
    ToggleSidebar, ToggleTerminal, ToggleViewMode, WorkbenchEvent,
};

use crate::tab_view::TabView;
use crate::workspace::{Locked, RightPanel, Workspace};

pub(crate) fn register(root: Div, cx: &Context<'_, Workspace>) -> Div {
    root.on_action(cx.listener(open_spotlight))
        .on_action(cx.listener(search_vault))
        .on_action(cx.listener(new_note))
        .on_action(cx.listener(toggle_sidebar))
        .on_action(cx.listener(toggle_terminal))
        .on_action(cx.listener(toggle_view_mode))
        .on_action(cx.listener(close_tab))
        .on_action(cx.listener(reopen_closed_tab))
        .on_action(cx.listener(toggle_pin_tab))
        .on_action(cx.listener(close_other_tabs))
        .on_action(cx.listener(close_all_tabs))
        .on_action(cx.listener(open_settings))
        .on_action(cx.listener(lock_vault))
        .on_action(cx.listener(sync_now))
        .on_action(cx.listener(toggle_backlinks))
        .on_action(cx.listener(toggle_history))
        .on_action(cx.listener(open_trash))
}

pub(crate) fn show_spotlight(
    this: &Workspace,
    mode: SpotlightMode,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    this.panes.update(cx, |panes, cx| panes.flush_all(cx));
    this.spotlight
        .update(cx, |spotlight, cx| spotlight.open(mode, window, cx));
    cx.notify();
}

fn open_spotlight(
    this: &mut Workspace,
    _: &OpenSpotlight,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    show_spotlight(this, SpotlightMode::Files, window, cx);
}

fn search_vault(
    this: &mut Workspace,
    _: &SearchVault,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    show_spotlight(this, SpotlightMode::FullText, window, cx);
}

fn new_note(
    this: &mut Workspace,
    _: &NewNote,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    if !this.layout.sidebar_open {
        this.layout.sidebar_open = true;
    }
    this.file_tree
        .update(cx, |tree, cx| tree.new_note(window, cx));
    cx.notify();
}

fn toggle_sidebar(
    this: &mut Workspace,
    _: &ToggleSidebar,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    this.layout.sidebar_open = !this.layout.sidebar_open;
    if !this.layout.sidebar_open {
        this.focus_active(window, cx);
    }
    this.schedule_save(window, cx);
    cx.notify();
}

fn toggle_terminal(
    this: &mut Workspace,
    _: &ToggleTerminal,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    this.layout.terminal_open = !this.layout.terminal_open;
    if this.layout.terminal_open {
        this.ensure_terminal(window, cx);
        let focus = gpui_kit::Focusable::focus_handle(this.terminal.read(cx), cx);
        focus.focus(window, cx);
    } else {
        this.focus_active(window, cx);
    }
    this.schedule_save(window, cx);
    cx.notify();
}

fn toggle_view_mode(
    this: &mut Workspace,
    _: &ToggleViewMode,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    if let Some(editor) = this.active_tab(cx).and_then(|view| view.note().cloned()) {
        editor.update(cx, NoteEditor::toggle_view_mode);
        this.schedule_save(window, cx);
        cx.notify();
    }
}

fn close_tab(
    this: &mut Workspace,
    _: &CloseTab,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    on_active_tab(this, window, cx, Panes::close);
}

fn reopen_closed_tab(
    this: &mut Workspace,
    _: &ReopenClosedTab,
    _: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    this.panes.update(cx, Panes::reopen_closed);
}

fn toggle_pin_tab(
    this: &mut Workspace,
    _: &TogglePinTab,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    on_active_tab(this, window, cx, Panes::toggle_pin);
}

fn close_other_tabs(
    this: &mut Workspace,
    _: &CloseOtherTabs,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    on_active_tab(this, window, cx, Panes::close_others);
}

fn close_all_tabs(
    this: &mut Workspace,
    _: &CloseAllTabs,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    on_active_tab(this, window, cx, |panes, pane, _, window, cx| {
        panes.close_all(pane, window, cx);
    });
}

fn on_active_tab(
    this: &Workspace,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
    act: impl FnOnce(&mut Panes<TabView>, usize, usize, &mut Window, &mut Context<'_, Panes<TabView>>),
) {
    this.panes.update(cx, |panes, cx| {
        if let Some((pane, tab)) = panes.active_position() {
            act(panes, pane, tab, window, cx);
        }
    });
}

fn open_settings(
    _: &mut Workspace,
    _: &OpenSettings,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    margin_preferences::open_settings_dialog(window, cx);
}

fn lock_vault(
    this: &mut Workspace,
    _: &LockVault,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    let closing = this.prepare_to_close(window, cx);
    cx.spawn(async move |this, cx| {
        closing.await;
        drop(this.update(cx, |_, cx| cx.emit(Locked)));
    })
    .detach();
}

fn sync_now(
    this: &mut Workspace,
    _: &SyncNow,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    this.sync_status
        .update(cx, |status, cx| status.sync_now(window, cx));
}

fn toggle_backlinks(
    this: &mut Workspace,
    _: &ToggleBacklinks,
    _: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    this.right_panel = if this.right_panel == Some(RightPanel::Backlinks) {
        None
    } else {
        Some(RightPanel::Backlinks)
    };
    cx.notify();
}

fn toggle_history(
    this: &mut Workspace,
    _: &ToggleHistory,
    _: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    this.right_panel = if this.right_panel == Some(RightPanel::History) {
        None
    } else {
        Some(RightPanel::History)
    };
    cx.notify();
}

fn open_trash(
    _: &mut Workspace,
    _: &OpenTrash,
    window: &mut Window,
    cx: &mut Context<'_, Workspace>,
) {
    let this = cx.entity().downgrade();
    margin_panels::open_trash_dialog(window, cx, move |event: WorkbenchEvent, window, cx| {
        drop(this.update(cx, |workspace, cx| {
            workspace.on_workbench_event(event, window, cx);
        }));
    });
}
