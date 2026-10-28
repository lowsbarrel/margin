use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{
    DeleteToBeginningOfLine, DeleteToPreviousWordStart, Input, InputEvent, InputState, MoveDown,
    MoveUp,
};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Icon, WindowExt, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled,
    Subscription, Task, Window, div,
};
use margin_dates::day_bucket;
use margin_i18n::{t, t_count, t_with};
use margin_kernel::{PageRequest, UnixMillis};
use margin_theme::MarginTheme;
use margin_trash::TrashItem;
use margin_workbench::{AppServices, CurrentVault, WorkbenchEvent, notify_error, run_blocking};

use crate::confirm_dialog::confirm_dialog;
use crate::list_key::{self, ListKey};
use crate::panel_header::{centered_message, panel_placeholder};
use crate::trash_filter::{matches, query_terms};
use crate::trash_row::{
    RowSelection, group_header, restored_notification, trash_footer, trash_row,
};
use crate::when::{self, Row};

const LIST_HEIGHT_SHARE: f32 = 0.52;

pub(crate) type TrashEventHandler = Rc<dyn Fn(WorkbenchEvent, &mut Window, &mut App)>;

pub(crate) struct TrashView {
    search: Entity<InputState>,
    on_event: TrashEventHandler,
    scroll: ScrollHandle,
    items: Vec<TrashItem>,
    loading: bool,
    selected: usize,
    busy: bool,
    now: UnixMillis,
    work: Task<()>,
    _search_events: Subscription,
}

impl TrashView {
    pub(crate) fn new(
        on_event: TrashEventHandler,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let placeholder = t(cx, "trash_search_placeholder");
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let search_events = cx.subscribe_in(&search, window, Self::on_search_event);
        let services = CurrentVault::expect(cx);
        let listed = run_blocking(cx, move || {
            services.trash.list(&PageRequest::first(usize::MAX)).items
        });
        let work = cx.spawn(async move |this, cx| {
            let items = listed.await;
            let _released = this.update(cx, |view, cx| {
                view.items = items;
                view.loading = false;
                view.now = cx.global::<AppServices>().clock.now();
                cx.notify();
            });
        });
        Self {
            search,
            on_event,
            scroll: ScrollHandle::new(),
            items: Vec::new(),
            loading: true,
            selected: 0,
            busy: false,
            now: cx.global::<AppServices>().clock.now(),
            work,
            _search_events: search_events,
        }
    }

    pub(crate) fn search(&self) -> Entity<InputState> {
        self.search.clone()
    }

    fn visible(&self, cx: &App) -> Vec<usize> {
        let terms = query_terms(&self.search.read(cx).value());
        (0..self.items.len())
            .filter(|index| matches(&self.items[*index], &terms))
            .collect()
    }

    fn active(&self, visible: &[usize]) -> usize {
        self.selected.min(visible.len().saturating_sub(1))
    }

    fn current(&self, cx: &App) -> Option<usize> {
        let visible = self.visible(cx);
        visible.get(self.active(&visible)).copied()
    }

    pub(crate) fn select(&mut self, index: usize, cx: &mut Context<'_, Self>) {
        if let Some(position) = self.visible(cx).iter().position(|item| *item == index) {
            self.selected = position;
            cx.notify();
        }
    }

    fn rows(&self, visible: &[usize]) -> Vec<Row<margin_dates::DayBucket>> {
        let offset = when::local_offset(self.now);
        when::grouped_rows(
            visible
                .iter()
                .map(|index| day_bucket(self.items[*index].deleted_at, self.now, offset)),
        )
    }

    fn step(&mut self, key: ListKey, cx: &mut Context<'_, Self>) {
        let visible = self.visible(cx);
        let Some(position) = list_key::step(Some(self.active(&visible)), visible.len(), key) else {
            return;
        };
        self.selected = position;
        let rows = self.rows(&visible);
        if let Some(child) = rows.iter().position(|row| *row == Row::Item(position)) {
            self.scroll.scroll_to_item(child);
        }
        cx.notify();
    }

    pub(crate) fn restore(&mut self, index: usize, window: &Window, cx: &mut Context<'_, Self>) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        let services = CurrentVault::expect(cx);
        let id = item.id.clone();
        let work = run_blocking(cx, move || services.trash.restore(&id));
        self.work = cx.spawn_in(window, async move |this, cx| {
            let restored = work.await;
            let _released = this.update_in(cx, |view, window, cx| {
                view.busy = false;
                match restored {
                    Ok(path) => {
                        view.items.retain(|other| other.id != item.id);
                        (view.on_event)(WorkbenchEvent::Reveal(path.clone()), window, cx);
                        let openable = (!item.is_dir).then_some(path);
                        let notification =
                            restored_notification(&item.name, openable, &view.on_event, cx);
                        window.push_notification(notification, cx);
                    }
                    Err(error) => notify_error(window, cx, "trash_restore_failed", &error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub(crate) fn confirm_delete(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if self.busy {
            return;
        }
        let title = t_with(cx, "trash_delete_confirm", &[("name", item.name.as_str())]);
        let body = t(cx, "trash_delete_confirm_body");
        let confirm = t(cx, "trash_delete_forever");
        let view = cx.entity().downgrade();
        confirm_dialog(window, cx, [title, body, confirm], move |window, cx| {
            let _released = view.update(cx, |view, cx| view.delete(&item, window, cx));
        });
    }

    fn delete(&mut self, item: &TrashItem, window: &Window, cx: &mut Context<'_, Self>) {
        self.busy = true;
        let services = CurrentVault::expect(cx);
        let item = item.clone();
        let id = item.id.clone();
        let work = run_blocking(cx, move || services.trash.delete(&id));
        self.work = cx.spawn_in(window, async move |this, cx| {
            let deleted = work.await;
            let _released = this.update_in(cx, |view, window, cx| {
                view.busy = false;
                match deleted {
                    Ok(()) => {
                        view.items.retain(|other| other.id != item.id);
                        let message =
                            t_with(cx, "trash_deleted_forever", &[("name", item.name.as_str())]);
                        window.push_notification(Notification::success(message), cx);
                    }
                    Err(error) => notify_error(window, cx, "trash_delete_failed", &error),
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub(crate) fn confirm_empty(window: &mut Window, cx: &mut Context<'_, Self>) {
        let title = t(cx, "trash_empty_confirm");
        let body = t(cx, "trash_empty_confirm_body");
        let confirm = t(cx, "trash_empty_trash");
        let view = cx.entity().downgrade();
        confirm_dialog(window, cx, [title, body, confirm], move |window, cx| {
            let _released = view.update(cx, |view, cx| view.empty(window, cx));
        });
    }

    fn empty(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        self.busy = true;
        let services = CurrentVault::expect(cx);
        let work = run_blocking(cx, move || services.trash.empty());
        self.work = cx.spawn_in(window, async move |this, cx| {
            let count = work.await;
            let _released = this.update_in(cx, |view, window, cx| {
                view.busy = false;
                view.items.clear();
                let message = t_count(cx, "trash_emptied", u64::from(count), &[]);
                window.push_notification(Notification::success(message), cx);
                cx.notify();
            });
        });
        cx.notify();
    }

    fn on_search_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        match event {
            InputEvent::Change => {
                self.selected = 0;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                if let Some(index) = self.current(cx) {
                    self.restore(index, window, cx);
                }
            }
            InputEvent::Focus | InputEvent::Blur => {}
        }
    }

    fn delete_shortcut(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if !window.modifiers().secondary(cx) {
            cx.propagate();
            return;
        }
        if let Some(index) = self.current(cx) {
            self.confirm_delete(index, window, cx);
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<'_, Self>) {
        let keystroke = &event.keystroke;
        let current = self.current(cx);
        match (ListKey::of(event, cx), current) {
            (Some(key @ (ListKey::Previous | ListKey::Next)), _) => self.step(key, cx),
            (Some(ListKey::Activate), Some(index)) => self.restore(index, window, cx),
            (Some(ListKey::Delete), Some(index)) if keystroke.key == "backspace" => {
                self.confirm_delete(index, window, cx);
            }
            (Some(_), _) => return,
            (None, _) => {
                let modifiers = keystroke.modifiers;
                let search_focused = self.search.read(cx).focus_handle(cx).is_focused(window);
                let Some(typed) = keystroke.key_char.clone().filter(|_| {
                    !search_focused && !modifiers.platform && !modifiers.control && !modifiers.alt
                }) else {
                    return;
                };
                self.search.update(cx, |search, cx| {
                    let query = format!("{}{typed}", search.value());
                    search.set_value(query, window, cx);
                    search.focus(window, cx);
                });
                self.selected = 0;
            }
        }
        cx.stop_propagation();
    }

    fn render_list(
        &self,
        visible: &[usize],
        window: &Window,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let terms = query_terms(&self.search.read(cx).value());
        let offset = when::local_offset(self.now);
        let active = self.active(visible);
        let mut children = Vec::new();
        for row in self.rows(visible) {
            let child = match row {
                Row::Group(bucket) => {
                    group_header(&when::day_label(bucket, self.now, offset, cx), cx)
                        .when(!children.is_empty(), Styled::pt_4)
                        .into_any_element()
                }
                Row::Item(position) => {
                    let index = visible[position];
                    trash_row(
                        index,
                        &self.items[index],
                        &terms,
                        self.now,
                        if position == active {
                            RowSelection::Selected
                        } else {
                            RowSelection::Unselected
                        },
                        self.busy,
                        cx,
                    )
                }
            };
            children.push(child);
        }
        div()
            .id("trash-list")
            .max_h(window.viewport_size().height * LIST_HEIGHT_SHARE)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p_1p5()
            .children(children)
            .into_any_element()
    }
}

impl Render for TrashView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx).clone();
        let root = v_flex()
            .id("trash-view")
            .key_context("TrashView")
            .gap_3()
            .capture_action(cx.listener(|view, _: &MoveUp, _, cx| view.step(ListKey::Previous, cx)))
            .capture_action(cx.listener(|view, _: &MoveDown, _, cx| view.step(ListKey::Next, cx)))
            .capture_action(
                cx.listener(|view, _: &DeleteToBeginningOfLine, window, cx| {
                    view.delete_shortcut(window, cx);
                }),
            )
            .capture_action(
                cx.listener(|view, _: &DeleteToPreviousWordStart, window, cx| {
                    view.delete_shortcut(window, cx);
                }),
            )
            .on_key_down(cx.listener(Self::on_key));
        if self.items.is_empty() {
            let message = if self.loading {
                panel_placeholder(t(cx, "trash_loading"), cx)
                    .h_40()
                    .into_any_element()
            } else {
                centered_message(
                    IconName::Trash,
                    t(cx, "trash_empty_state"),
                    t(cx, "trash_empty_state_hint"),
                    cx,
                )
            };
            return root.child(message);
        }
        let visible = self.visible(cx);
        let query = self.search.read(cx).value();
        let list = if visible.is_empty() {
            let hint = t_with(cx, "trash_no_matches_hint", &[("query", query.trim())]);
            centered_message(IconName::SearchX, t(cx, "trash_no_matches"), hint, cx)
        } else {
            self.render_list(&visible, window, cx)
        };
        let shown = (!query.trim().is_empty()).then_some(visible.len());
        root.child(
            Input::new(&self.search)
                .prefix(Icon::new(IconName::Search).text_color(palette.text_tertiary))
                .cleanable(true),
        )
        .child(list)
        .child(trash_footer(self.items.len(), shown, self.busy, cx))
    }
}
