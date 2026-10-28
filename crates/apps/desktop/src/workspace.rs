use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{
    App, AppContext, Context, Entity, EntityId, EventEmitter, FocusHandle, Focusable, Subscription,
    Task, Window,
};
use margin_editor_assets::VaultIndex;
use margin_file_tree::FileTree;
use margin_kernel::{RelativePath, Subscription as WatchSubscription, Watcher};
use margin_panels::{BacklinksPanel, HistoryPanel, PanelEvent, SyncStatus};
use margin_panes::{PaneItem, Panes};
use margin_spotlight::{Spotlight, SpotlightEvent, SpotlightMode};
use margin_terminal::{TerminalDock, TerminalDockEvent};
use margin_theme::MarginTheme;
use margin_workbench::{
    FileKind, OpenRequest, VaultServices, WorkbenchEvent, notify_error, run_blocking,
};

use crate::shell_layout::ShellEdge;
use crate::tab_view::TabView;
use crate::workspace_state::{self, ShellLayout, Snapshot};
use crate::{lifecycle, pane_events, workspace_actions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RightPanel {
    Backlinks,
    History,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Locked;

pub(crate) struct Workspace {
    pub services: Arc<VaultServices>,
    pub focus: FocusHandle,
    pub panes: Entity<Panes<TabView>>,
    pub tab_events: HashMap<EntityId, Subscription>,
    pub file_tree: Entity<FileTree>,
    pub spotlight: Entity<Spotlight>,
    pub terminal: Entity<TerminalDock>,
    pub backlinks: Entity<BacklinksPanel>,
    pub history: Entity<HistoryPanel>,
    pub sync_status: Entity<SyncStatus>,
    pub right_panel: Option<RightPanel>,
    pub layout: ShellLayout,
    pub resizing: Option<ShellEdge>,
    pub restored: bool,
    pub save_task: Option<Task<()>>,
    pub watch: Option<WatchSubscription>,
    pub tasks: Vec<Task<()>>,
    pub _subscriptions: Vec<Subscription>,
}

impl EventEmitter<Locked> for Workspace {}

impl Workspace {
    pub(crate) fn new(
        services: Arc<VaultServices>,
        watcher: &Arc<dyn Watcher>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let file_tree = cx.new(|cx| FileTree::new(window, cx));
        let spotlight = cx.new(|cx| Spotlight::new(window, cx));
        let terminal = cx.new(|cx| TerminalDock::new(window, cx));
        let backlinks = cx.new(|cx| BacklinksPanel::new(window, cx));
        let history = cx.new(|cx| HistoryPanel::new(window, cx));
        let sync_status = cx.new(|cx| SyncStatus::new(window, cx));
        let panes = cx.new(|cx| Panes::new(cx));
        let subscriptions = vec![
            cx.subscribe_in(&panes, window, pane_events::on_event),
            cx.subscribe_in(&file_tree, window, Self::forward),
            cx.subscribe_in(&spotlight, window, Self::forward),
            cx.subscribe_in(&backlinks, window, Self::forward),
            cx.subscribe_in(&history, window, Self::history_event),
            cx.subscribe_in(&sync_status, window, Self::forward),
            cx.subscribe_in(
                &spotlight,
                window,
                |this, _, _: &SpotlightEvent, window, cx| {
                    this.focus_active(window, cx);
                    cx.notify();
                },
            ),
            cx.subscribe_in(
                &terminal,
                window,
                |this, _, _: &TerminalDockEvent, window, cx| {
                    this.layout.terminal_open = false;
                    this.focus_active(window, cx);
                    this.schedule_save(window, cx);
                },
            ),
            cx.subscribe_in(&backlinks, window, |this, _, _: &PanelEvent, window, cx| {
                this.close_right_panel(window, cx);
            }),
            cx.subscribe_in(&history, window, |this, _, _: &PanelEvent, window, cx| {
                this.close_right_panel(window, cx);
            }),
            cx.observe_global_in::<MarginTheme>(window, |this, _, cx| {
                lifecycle::persist_theme(&this.services, cx);
            }),
        ];
        let mut this = Self {
            panes,
            tab_events: HashMap::new(),
            focus: cx.focus_handle(),
            file_tree,
            spotlight,
            terminal,
            backlinks,
            history,
            sync_status,
            right_panel: None,
            layout: ShellLayout::default(),
            resizing: None,
            restored: false,
            save_task: None,
            watch: None,
            tasks: Vec::new(),
            _subscriptions: subscriptions,
            services,
        };
        lifecycle::start(&mut this, watcher, window, cx);
        this
    }

    pub(crate) fn track(&mut self, task: Task<()>) {
        self.tasks.push(task);
    }

    pub(crate) fn set_watch(&mut self, watch: WatchSubscription) {
        self.watch = Some(watch);
    }

    pub(crate) fn restore(
        &mut self,
        state: Option<margin_settings::WorkspaceState>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(state) = state {
            let snapshot = workspace_state::apply(&state, &self.panes, window, cx);
            self.layout = snapshot.layout;
            self.file_tree.update(cx, |tree, cx| {
                tree.set_sort_by(snapshot.sort_by, cx);
                tree.set_expanded_folders(snapshot.expanded_folders, cx);
            });
            if self.layout.terminal_open {
                self.ensure_terminal(window, cx);
            }
            self.focus_active(window, cx);
        }
        self.restored = true;
        self.active_changed(cx);
    }

    fn forward<E>(
        &mut self,
        _: &Entity<E>,
        event: &WorkbenchEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.on_workbench_event(event.clone(), window, cx);
    }

    fn history_event(
        &mut self,
        _: &Entity<HistoryPanel>,
        event: &WorkbenchEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let WorkbenchEvent::Saved(path) = event {
            let changed = [path.clone()];
            self.panes
                .update(cx, |panes, cx| panes.reload(&changed, cx));
        }
        self.on_workbench_event(event.clone(), window, cx);
    }

    pub(crate) fn active_tab(&self, cx: &App) -> Option<TabView> {
        self.panes.read(cx).active_item().cloned()
    }

    pub(crate) fn focus_active(&self, window: &mut Window, cx: &mut App) {
        self.panes
            .update(cx, |panes, cx| panes.focus_active(window, cx));
    }

    fn close_right_panel(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.right_panel = None;
        self.focus_active(window, cx);
        cx.notify();
    }

    pub(crate) fn on_workbench_event(
        &mut self,
        event: WorkbenchEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            WorkbenchEvent::Open(request) => self.open(request, window, cx),
            WorkbenchEvent::Renamed { from, to } => {
                self.panes
                    .update(cx, |panes, cx| panes.renamed(&from, &to, cx));
                self.reindex(vec![from, to], cx);
                self.refresh_vault(cx);
                self.active_changed(cx);
                self.sync_status.update(cx, SyncStatus::mark_local_change);
            }
            WorkbenchEvent::Deleted(path) => {
                self.panes
                    .update(cx, |panes, cx| panes.deleted(&path, window, cx));
                self.reindex(vec![path], cx);
                self.refresh_vault(cx);
                self.active_changed(cx);
                self.sync_status.update(cx, SyncStatus::mark_local_change);
            }
            WorkbenchEvent::Saved(path) => {
                self.reindex(vec![path], cx);
                self.refresh_panels(cx);
                VaultIndex::refresh(cx);
                self.sync_status.update(cx, SyncStatus::mark_local_change);
            }
            WorkbenchEvent::TagSelected(tag) => {
                workspace_actions::show_spotlight(self, SpotlightMode::Tag(tag), window, cx);
            }
            WorkbenchEvent::TreeChanged => self.refresh_vault(cx),
            WorkbenchEvent::Reveal(path) => {
                self.refresh_vault(cx);
                self.file_tree
                    .update(cx, |tree, cx| tree.reveal(&path, window, cx));
            }
        }
        self.schedule_save(window, cx);
        cx.notify();
    }

    pub(crate) fn open(&mut self, request: OpenRequest, window: &Window, cx: &Context<'_, Self>) {
        let services = Arc::clone(&self.services);
        let path = request.path.clone();
        let created = run_blocking(cx, move || {
            if FileKind::of(&path) != FileKind::Note || services.files.exists(&path) {
                return Ok(false);
            }
            if let Some(parent) = path.parent() {
                services.files.create_directory(&parent)?;
            }
            services.files.write_file(&path, b"").map(|()| true)
        });
        self.tasks.push(cx.spawn_in(window, async move |this, cx| {
            let created = created.await;
            drop(this.update_in(cx, |this, window, cx| match created {
                Ok(created) => {
                    if created {
                        this.refresh_vault(cx);
                    }
                    this.show(&request, window, cx);
                }
                Err(error) => notify_error(window, cx, "toast_cannot_open_file", &error),
            }));
        }));
    }

    fn show(&self, request: &OpenRequest, window: &mut Window, cx: &mut Context<'_, Self>) {
        let view = pane_events::show(self, request, window, cx);
        if let (Some(anchor), Some(editor)) = (&request.anchor, view.note()) {
            editor.update(cx, |editor, cx| editor.scroll_to_anchor(anchor, window, cx));
        }
    }

    fn reindex(&self, paths: Vec<RelativePath>, cx: &App) {
        let services = Arc::clone(&self.services);
        run_blocking(cx, move || {
            services.files.invalidate_search_cache();
            drop(services.search.apply_changes(&paths));
        })
        .detach();
    }

    pub(crate) fn on_vault_changed(&self, paths: Vec<RelativePath>, cx: &mut Context<'_, Self>) {
        self.panes.update(cx, |panes, cx| panes.reload(&paths, cx));
        self.reindex(paths, cx);
        self.refresh_vault(cx);
        self.refresh_panels(cx);
        self.sync_status.update(cx, SyncStatus::mark_local_change);
    }

    pub(crate) fn refresh_vault(&self, cx: &mut App) {
        self.file_tree.update(cx, |tree, cx| tree.refresh(cx));
        VaultIndex::refresh(cx);
    }

    fn refresh_panels(&self, cx: &mut App) {
        self.backlinks.update(cx, BacklinksPanel::refresh);
        self.history.update(cx, HistoryPanel::refresh);
    }

    pub(crate) fn active_changed(&self, cx: &mut Context<'_, Self>) {
        let active = self.active_tab(cx);
        let path = active.as_ref().map(|view| view.handle().path(cx));
        let note = path
            .clone()
            .filter(|_| active.is_some_and(|view| view.note().is_some()));
        self.backlinks
            .update(cx, |panel, cx| panel.set_note(note.clone(), cx));
        self.history
            .update(cx, |panel, cx| panel.set_note(note, cx));
        self.file_tree
            .update(cx, |tree, cx| tree.set_active_file(path, cx));
        cx.notify();
    }

    pub(crate) fn ensure_terminal(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if !self.terminal.read(cx).has_terminals() {
            self.terminal
                .update(cx, |dock, cx| dock.new_terminal(window, cx));
        }
    }

    pub(crate) fn has_unsaved_tabs(&self, cx: &App) -> bool {
        self.panes
            .read(cx)
            .layout()
            .items()
            .any(|view| view.is_dirty(cx))
    }

    pub(crate) fn snapshot(&self, cx: &App) -> Snapshot {
        let tree = self.file_tree.read(cx);
        Snapshot {
            layout: self.layout,
            expanded_folders: tree.expanded_folders(),
            sort_by: tree.sort_by(),
        }
    }

    pub(crate) fn schedule_save(&mut self, window: &Window, cx: &Context<'_, Self>) {
        if !self.restored {
            return;
        }
        self.save_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(lifecycle::WORKSPACE_SAVE_DELAY)
                .await;
            drop(this.update(cx, |this, cx| this.save_now(cx).detach()));
        }));
    }

    pub(crate) fn save_now(&mut self, cx: &Context<'_, Self>) -> Task<()> {
        self.save_task = None;
        if !self.restored {
            return Task::ready(());
        }
        let state = workspace_state::capture(self.panes.read(cx).layout(), &self.snapshot(cx), cx);
        let services = Arc::clone(&self.services);
        run_blocking(cx, move || {
            drop(services.settings.save_workspace(&services.key, &state));
        })
    }

    pub(crate) fn prepare_to_close(
        &mut self,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) -> Task<()> {
        self.panes.update(cx, |panes, cx| panes.flush_all(cx));
        let saved = self.save_now(cx);
        self.restored = false;
        self.watch = None;
        cx.spawn_in(window, async move |this, cx| {
            saved.await;
            lifecycle::wait_until_clean(this, cx).await;
        })
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
