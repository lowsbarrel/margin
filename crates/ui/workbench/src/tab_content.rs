use std::sync::Arc;

use gpui_kit::component::dock::{Panel, PanelView};
use gpui_kit::{App, Context, Entity, EventEmitter};
use margin_kernel::RelativePath;

use crate::workbench_event::WorkbenchEvent;

pub trait TabContent: Panel + EventEmitter<WorkbenchEvent> {
    fn path(&self) -> &RelativePath;

    fn rename(&mut self, to: RelativePath, cx: &mut Context<Self>);

    fn flush(&mut self, cx: &mut Context<Self>);

    fn reload(&mut self, cx: &mut Context<Self>);
}

pub trait TabHandle {
    fn path(&self, cx: &App) -> RelativePath;

    fn rename(&self, to: RelativePath, cx: &mut App);

    fn flush(&self, cx: &mut App);

    fn reload(&self, cx: &mut App);

    fn panel(&self) -> Arc<dyn PanelView>;
}

impl<T: TabContent> TabHandle for Entity<T> {
    fn path(&self, cx: &App) -> RelativePath {
        self.read(cx).path().clone()
    }

    fn rename(&self, to: RelativePath, cx: &mut App) {
        self.update(cx, |content, cx| content.rename(to, cx));
    }

    fn flush(&self, cx: &mut App) {
        self.update(cx, |content, cx| content.flush(cx));
    }

    fn reload(&self, cx: &mut App) {
        self.update(cx, |content, cx| content.reload(cx));
    }

    fn panel(&self) -> Arc<dyn PanelView> {
        Arc::new(self.clone())
    }
}
