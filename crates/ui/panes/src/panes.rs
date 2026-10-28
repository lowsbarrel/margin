use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use gpui_kit::{
    App, Bounds, Context, EntityId, EventEmitter, FocusHandle, Pixels, ScrollHandle, Subscription,
    Window,
};
use margin_kernel::RelativePath;
use margin_workbench::TabHandle;

use crate::closed_tabs::ClosedTabs;
use crate::drag::Dragged;
use crate::layout::{PaneId, PaneLayout, Side, Tab};

pub trait PaneItem: Clone + 'static {
    fn handle(&self) -> &dyn TabHandle;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanesEvent {
    Changed,
    Open {
        path: RelativePath,
        pane: usize,
        at: Option<usize>,
    },
    OpenSplit {
        path: RelativePath,
        pane: usize,
        side: Side,
    },
}

#[derive(Debug, Default)]
pub(crate) struct Geometry {
    pub strips: HashMap<PaneId, Bounds<Pixels>>,
    pub row: Vec<Bounds<Pixels>>,
}

#[expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "the strip, zone, drop and view modules of this crate render and mutate the pane state"
)]
pub struct Panes<T: PaneItem> {
    pub(crate) layout: PaneLayout<T>,
    pub(crate) closed: ClosedTabs,
    pub(crate) focus: FocusHandle,
    pub(crate) dragging: Option<Dragged>,
    pub(crate) strip_target: Option<(usize, usize)>,
    pub(crate) resizing: Option<usize>,
    pub(crate) geometry: Rc<RefCell<Geometry>>,
    pub(crate) scrolls: HashMap<PaneId, ScrollHandle>,
    watches: HashMap<EntityId, Subscription>,
}

impl<T: PaneItem> EventEmitter<PanesEvent> for Panes<T> {}

impl<T: PaneItem> fmt::Debug for Panes<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Panes")
            .field("panes", &self.layout.panes().len())
            .field("dragging", &self.dragging)
            .finish_non_exhaustive()
    }
}

impl<T: PaneItem> Panes<T> {
    #[must_use]
    pub fn new(cx: &Context<'_, Self>) -> Self {
        let layout = PaneLayout::default();
        let scrolls = layout
            .panes()
            .iter()
            .map(|pane| (pane.id, ScrollHandle::new()))
            .collect();
        Self {
            layout,
            closed: ClosedTabs::default(),
            focus: cx.focus_handle(),
            dragging: None,
            strip_target: None,
            resizing: None,
            geometry: Rc::default(),
            scrolls,
            watches: HashMap::new(),
        }
    }

    #[must_use]
    pub const fn layout(&self) -> &PaneLayout<T> {
        &self.layout
    }

    #[must_use]
    pub fn active_item(&self) -> Option<&T> {
        self.layout.active_tab().map(|tab| &tab.item)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layout.items().next().is_none()
    }

    #[must_use]
    pub fn can_reopen(&self) -> bool {
        !self.closed.is_empty()
    }

    pub fn open(
        &mut self,
        pane: Option<usize>,
        at: Option<usize>,
        path: &RelativePath,
        make: impl FnOnce(&mut Window, &mut App) -> T,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> T {
        let pane = pane
            .unwrap_or_else(|| self.layout.active_pane())
            .min(self.layout.panes().len() - 1);
        let existing = self.layout.panes()[pane]
            .tabs
            .iter()
            .position(|tab| tab.item.handle().path(cx) == *path);
        if let Some(tab) = existing {
            self.activate(pane, tab, window, cx);
            return self.layout.panes()[pane].tabs[tab].item.clone();
        }
        let item = make(window, cx);
        self.watch(&item, window, cx);
        self.layout.open(pane, at, item.clone());
        self.changed(window, cx);
        item
    }

    pub fn open_split(
        &mut self,
        reference: usize,
        side: Side,
        item: T,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.watch(&item, window, cx);
        self.layout.open_split(reference, side, item);
        self.changed(window, cx);
    }

    pub fn restore(
        &mut self,
        panes: Vec<(Vec<Tab<T>>, usize)>,
        flexes: &[f32],
        active: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        for tab in panes.iter().flat_map(|(tabs, _)| tabs) {
            self.watch(&tab.item, window, cx);
        }
        self.layout.restore(panes, flexes, active);
        self.changed(window, cx);
    }

    fn watch(&mut self, item: &T, window: &mut Window, cx: &mut Context<'_, Self>) {
        let id = item.handle().id();
        let focus = item.handle().focus_handle(cx);
        let watch = cx.on_focus_in(&focus, window, move |this, window, cx| {
            this.item_focused(id, window, cx);
        });
        self.watches.insert(id, watch);
    }

    fn item_focused(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some((pane, tab)) = self.position_of(id) else {
            return;
        };
        let current =
            self.layout.active_pane() == pane && self.layout.panes()[pane].active == Some(tab);
        if !current {
            self.layout.focus_pane(pane);
            self.layout.switch(pane, tab);
            self.changed(window, cx);
        }
    }

    pub(crate) fn position_of(&self, id: EntityId) -> Option<(usize, usize)> {
        self.layout.position(|item| item.handle().id() == id)
    }

    pub(crate) fn changed(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let live: Vec<EntityId> = self.layout.items().map(|item| item.handle().id()).collect();
        self.watches.retain(|id, _| live.contains(id));
        let panes: Vec<PaneId> = self.layout.panes().iter().map(|pane| pane.id).collect();
        self.scrolls.retain(|id, _| panes.contains(id));
        for pane in self.layout.panes() {
            let scroll = self.scrolls.entry(pane.id).or_default();
            if let Some(active) = pane.active {
                scroll.scroll_to_item(active);
            }
        }
        cx.emit(PanesEvent::Changed);
        cx.notify();
        window.refresh();
    }

    pub fn activate(
        &mut self,
        pane: usize,
        tab: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.layout.focus_pane(pane);
        self.layout.switch(pane, tab);
        self.focus_active(window, cx);
        self.changed(window, cx);
    }

    pub fn focus_pane(&mut self, pane: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.layout.focus_pane(pane) {
            self.focus_active(window, cx);
            self.changed(window, cx);
        }
    }

    pub fn focus_active(&self, window: &mut Window, cx: &mut App) {
        match self.active_item() {
            Some(item) => item.handle().focus(window, cx),
            None => self.focus.focus(window, cx),
        }
    }

    pub(crate) fn retire(
        &mut self,
        closed: Vec<T>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let had_focus = closed
            .iter()
            .any(|item| item.handle().is_focused(window, cx));
        for item in closed {
            item.handle().flush(cx);
            self.closed.push(item.handle().path(cx));
        }
        if had_focus {
            self.focus_active(window, cx);
        }
        self.changed(window, cx);
    }

    pub fn close(
        &mut self,
        pane: usize,
        tab: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let closed = self.layout.close(pane, tab);
        self.retire(closed, window, cx);
    }

    pub fn close_others(
        &mut self,
        pane: usize,
        tab: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let closed = self.layout.close_others(pane, tab);
        self.retire(closed, window, cx);
    }

    pub fn close_all(&mut self, pane: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        let closed = self.layout.close_all(pane);
        self.retire(closed, window, cx);
    }

    pub fn close_pane(&mut self, pane: usize, window: &mut Window, cx: &mut Context<'_, Self>) {
        let closed = self.layout.close_pane(pane);
        let had_focus = closed
            .iter()
            .any(|item| item.handle().is_focused(window, cx));
        for item in &closed {
            item.handle().flush(cx);
        }
        if had_focus {
            self.focus_active(window, cx);
        }
        self.changed(window, cx);
    }

    pub fn toggle_pin(
        &mut self,
        pane: usize,
        tab: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.layout.toggle_pin(pane, tab);
        self.changed(window, cx);
    }

    #[must_use]
    pub fn active_position(&self) -> Option<(usize, usize)> {
        let pane = self.layout.active_pane();
        self.layout.panes()[pane].active.map(|tab| (pane, tab))
    }

    pub fn reopen_closed(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(path) = self.closed.take() {
            let pane = self.layout.active_pane();
            cx.emit(PanesEvent::Open {
                path,
                pane,
                at: None,
            });
        }
    }

    pub fn renamed(&mut self, from: &RelativePath, to: &RelativePath, cx: &mut Context<'_, Self>) {
        let remap = |path: &RelativePath| {
            if path == from {
                Some(to.clone())
            } else {
                path.strip_prefix(from)
                    .and_then(|rest| to.join(rest.as_str()).ok())
            }
        };
        for item in self.layout.items() {
            if let Some(next) = remap(&item.handle().path(cx)) {
                item.handle().rename(next, cx);
            }
        }
        self.closed.rename(remap);
        cx.notify();
    }

    pub fn deleted(
        &mut self,
        path: &RelativePath,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let paths: HashMap<EntityId, RelativePath> = self
            .layout
            .items()
            .map(|item| (item.handle().id(), item.handle().path(cx)))
            .collect();
        let had_focus = self
            .layout
            .items()
            .any(|item| item.handle().is_focused(window, cx));
        let removed = self.layout.remove_where(|item| {
            paths
                .get(&item.handle().id())
                .is_some_and(|open| open.starts_with(path))
        });
        self.closed.forget(|closed| closed.starts_with(path));
        if removed.is_empty() {
            return;
        }
        if had_focus
            && self
                .active_item()
                .is_none_or(|item| !item.handle().is_focused(window, cx))
        {
            self.focus_active(window, cx);
        }
        self.changed(window, cx);
    }

    pub fn flush_all(&self, cx: &mut App) {
        for item in self.layout.items() {
            item.handle().flush(cx);
        }
    }

    pub fn reload(&self, changed: &[RelativePath], cx: &mut App) {
        for item in self.layout.items() {
            if changed.contains(&item.handle().path(cx)) {
                item.handle().reload(cx);
            }
        }
    }
}
