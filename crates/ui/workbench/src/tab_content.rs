use gpui_kit::{
    AnyView, App, Context, Entity, EntityId, EventEmitter, FocusHandle, Focusable, Render, Window,
};
use margin_kernel::RelativePath;

use crate::workbench_event::WorkbenchEvent;

pub trait TabContent: Render + Focusable + EventEmitter<WorkbenchEvent> {
    fn path(&self) -> &RelativePath;

    fn rename(&mut self, to: RelativePath, cx: &mut Context<'_, Self>);

    fn flush(&mut self, cx: &mut Context<'_, Self>);

    fn reload(&mut self, cx: &mut Context<'_, Self>);
}

pub trait TabHandle {
    fn id(&self) -> EntityId;

    fn view(&self) -> AnyView;

    fn path(&self, cx: &App) -> RelativePath;

    fn rename(&self, to: RelativePath, cx: &mut App);

    fn flush(&self, cx: &mut App);

    fn reload(&self, cx: &mut App);

    fn focus_handle(&self, cx: &App) -> FocusHandle;

    fn focus(&self, window: &mut Window, cx: &mut App);

    fn is_focused(&self, window: &Window, cx: &App) -> bool;
}

impl<T: TabContent> TabHandle for Entity<T> {
    fn id(&self) -> EntityId {
        self.entity_id()
    }

    fn view(&self) -> AnyView {
        self.clone().into()
    }

    fn path(&self, cx: &App) -> RelativePath {
        self.read(cx).path().clone()
    }

    fn rename(&self, to: RelativePath, cx: &mut App) {
        self.update(cx, |content, cx| content.rename(to, cx));
    }

    fn flush(&self, cx: &mut App) {
        self.update(cx, TabContent::flush);
    }

    fn reload(&self, cx: &mut App) {
        self.update(cx, TabContent::reload);
    }

    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.read(cx).focus_handle(cx)
    }

    fn focus(&self, window: &mut Window, cx: &mut App) {
        self.read(cx).focus_handle(cx).focus(window, cx);
    }

    fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.read(cx).focus_handle(cx).contains_focused(window, cx)
    }
}
