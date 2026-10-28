use std::fmt;

use gpui_kit::assets::IconName;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{WindowExt, v_flex};
use gpui_kit::{
    App, ClickEvent, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, ScrollHandle, StatefulInteractiveElement,
    Styled, Task, Window, div,
};
use margin_history::Snapshot;
use margin_i18n::{t, t_count, t_with};
use margin_kernel::RelativePath;
use margin_theme::MarginTheme;
use margin_workbench::{CurrentVault, WorkbenchEvent, run_blocking};

use crate::confirm_dialog::confirm_dialog;
use crate::diff_view::PreviewDiff;
use crate::history_list::{history_list, history_rows};
use crate::history_tasks::{compare_with_current, restore_snapshot};
use crate::list_key::{self, ListKey};
use crate::panel_event::PanelEvent;
use crate::panel_header::{icon_button, panel_header, panel_placeholder};
use crate::when::Row;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Loading,
    Ready,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Preview {
    filename: String,
    diff: PreviewDiff,
}

pub struct HistoryPanel {
    focus_handle: FocusHandle,
    scroll: ScrollHandle,
    note: Option<RelativePath>,
    snapshots: Vec<Snapshot>,
    status: Status,
    selected: Option<usize>,
    preview: Option<Preview>,
    restoring: Option<usize>,
    revision: u64,
    load: Task<()>,
    work: Task<()>,
}

impl HistoryPanel {
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "every panel is built through the Shell's window-and-context constructor"
    )]
    pub fn new(_window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            note: None,
            snapshots: Vec::new(),
            status: Status::Ready,
            selected: None,
            preview: None,
            restoring: None,
            revision: 0,
            load: Task::ready(()),
            work: Task::ready(()),
        }
    }

    pub fn set_note(&mut self, path: Option<RelativePath>, cx: &mut Context<'_, Self>) {
        if self.note == path {
            return;
        }
        self.note = path;
        self.snapshots.clear();
        self.selected = None;
        self.preview = None;
        self.refresh(cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<'_, Self>) {
        self.revision += 1;
        let Some(note) = self.note.clone() else {
            self.status = Status::Ready;
            cx.notify();
            return;
        };
        self.status = Status::Loading;
        let revision = self.revision;
        let services = CurrentVault::expect(cx);
        let work = run_blocking(cx, move || services.history.list(&note));
        self.load = cx.spawn(async move |this, cx| {
            let listed = work.await;
            let _released = this.update(cx, |panel, cx| {
                if panel.revision != revision {
                    return;
                }
                match listed {
                    Ok(snapshots) => {
                        panel.selected = panel.selected.filter(|index| *index < snapshots.len());
                        panel.preview = None;
                        panel.snapshots = snapshots;
                        panel.status = Status::Ready;
                    }
                    Err(_) => panel.status = Status::Failed,
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub(crate) fn click_snapshot(
        &mut self,
        index: usize,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.toggle_preview(index, window, cx);
        self.selected = self.preview.is_some().then_some(index);
    }

    fn toggle_preview(&mut self, index: usize, window: &Window, cx: &mut Context<'_, Self>) {
        let (Some(note), Some(snapshot)) = (self.note.clone(), self.snapshots.get(index)) else {
            return;
        };
        let filename = snapshot.filename.clone();
        if self
            .preview
            .as_ref()
            .is_some_and(|preview| preview.filename == filename)
        {
            self.preview = None;
            cx.notify();
            return;
        }
        self.preview = Some(Preview {
            filename: filename.clone(),
            diff: PreviewDiff::Loading,
        });
        let revision = self.revision;
        let services = CurrentVault::expect(cx);
        let wanted = filename.clone();
        let work = run_blocking(cx, move || compare_with_current(&services, &note, &wanted));
        self.work = cx.spawn_in(window, async move |this, cx| {
            let compared = work.await;
            let _released = this.update_in(cx, |panel, window, cx| {
                let current = panel
                    .preview
                    .as_ref()
                    .map(|preview| preview.filename.as_str());
                if panel.revision != revision || current != Some(filename.as_str()) {
                    return;
                }
                if let Ok(comparison) = compared {
                    panel.preview = Some(Preview {
                        filename,
                        diff: comparison.into_preview(),
                    });
                } else {
                    panel.preview = None;
                    notify_error(window, cx, "history_read_failed");
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub(crate) fn restore(&mut self, index: usize, window: &Window, cx: &mut Context<'_, Self>) {
        let (Some(note), Some(snapshot)) = (self.note.clone(), self.snapshots.get(index)) else {
            return;
        };
        if self.restoring.is_some() {
            return;
        }
        self.restoring = Some(index);
        let filename = snapshot.filename.clone();
        let services = CurrentVault::expect(cx);
        let target = note.clone();
        let work = run_blocking(cx, move || restore_snapshot(&services, &target, &filename));
        self.work = cx.spawn_in(window, async move |this, cx| {
            let restored = work.await;
            let _released = this.update_in(cx, |panel, window, cx| {
                panel.restoring = None;
                if restored {
                    cx.emit(WorkbenchEvent::Saved(note));
                    window.push_notification(Notification::success(t(cx, "history_restored")), cx);
                    panel.refresh(cx);
                } else {
                    notify_error(window, cx, "history_restore_failed");
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub(crate) fn delete(&mut self, index: usize, window: &Window, cx: &Context<'_, Self>) {
        let (Some(note), Some(snapshot)) = (self.note.clone(), self.snapshots.get(index)) else {
            return;
        };
        let filename = snapshot.filename.clone();
        let services = CurrentVault::expect(cx);
        let wanted = filename.clone();
        let work = run_blocking(cx, move || services.history.delete(&note, &wanted));
        self.work = cx.spawn_in(window, async move |this, cx| {
            let deleted = work.await;
            let _released = this.update_in(cx, |panel, window, cx| {
                if deleted.is_err() {
                    notify_error(window, cx, "history_delete_failed");
                    return;
                }
                panel.snapshots.retain(|item| item.filename != filename);
                if panel
                    .preview
                    .as_ref()
                    .is_some_and(|preview| preview.filename == filename)
                {
                    panel.preview = None;
                }
                panel.selected =
                    list_key::step(panel.selected, panel.snapshots.len(), ListKey::Activate)
                        .filter(|_| panel.selected.is_some());
                cx.notify();
            });
        });
    }

    fn confirm_clear(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(note) = self.note.clone() else {
            return;
        };
        let title = t_with(cx, "history_clear_confirm", &[("name", note.file_stem())]);
        let body = t(cx, "history_clear_confirm_body");
        let confirm = t(cx, "history_clear_all");
        let panel = cx.entity().downgrade();
        confirm_dialog(window, cx, [title, body, confirm], move |window, cx| {
            let _released = panel.update(cx, |panel, cx| panel.clear(window, cx));
        });
    }

    fn clear(&mut self, window: &Window, cx: &Context<'_, Self>) {
        let Some(note) = self.note.clone() else {
            return;
        };
        let services = CurrentVault::expect(cx);
        let work = run_blocking(cx, move || services.history.clear(&note));
        self.work = cx.spawn_in(window, async move |this, cx| {
            let cleared = work.await;
            let _released = this.update_in(cx, |panel, window, cx| {
                let Ok(count) = cleared else {
                    notify_error(window, cx, "history_delete_failed");
                    return;
                };
                panel.snapshots.clear();
                panel.preview = None;
                panel.selected = None;
                let message = t_count(cx, "history_cleared", u64::from(count), &[]);
                window.push_notification(Notification::success(message), cx);
                cx.notify();
            });
        });
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(key) = ListKey::of(event, cx) else {
            return;
        };
        cx.stop_propagation();
        match (key, self.selected) {
            (ListKey::Close, _) => cx.emit(PanelEvent::Close),
            (ListKey::Previous | ListKey::Next, _) => {
                self.selected = list_key::step(self.selected, self.snapshots.len(), key);
                if let Some(child) = self.selected.and_then(|index| self.child_index(index, cx)) {
                    self.scroll.scroll_to_item(child);
                }
                cx.notify();
            }
            (ListKey::Activate, Some(index)) => self.toggle_preview(index, window, cx),
            (ListKey::Commit, Some(index)) => self.restore(index, window, cx),
            (ListKey::Delete, Some(index)) => self.delete(index, window, cx),
            (_, None) => {}
        }
    }

    fn child_index(&self, index: usize, cx: &App) -> Option<usize> {
        history_rows(&self.snapshots, cx)
            .0
            .iter()
            .position(|row| *row == Row::Item(index))
    }
}

fn notify_error(window: &mut Window, cx: &mut App, key: &str) {
    window.push_notification(Notification::error(t(cx, key)), cx);
}

impl fmt::Debug for HistoryPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HistoryPanel").finish_non_exhaustive()
    }
}

impl EventEmitter<WorkbenchEvent> for HistoryPanel {}

impl EventEmitter<PanelEvent> for HistoryPanel {}

impl Focusable for HistoryPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HistoryPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx).clone();
        let mut actions = Vec::new();
        if !self.snapshots.is_empty() {
            actions.push(
                icon_button("history-clear", IconName::Trash, t(cx, "history_clear_all")).on_click(
                    cx.listener(|panel, _: &ClickEvent, window, cx| {
                        panel.confirm_clear(window, cx);
                    }),
                ),
            );
        }
        actions.push(
            icon_button("history-close", IconName::X, t(cx, "history_close"))
                .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(PanelEvent::Close))),
        );
        let header = panel_header(
            IconName::RotateCcwClock,
            &t(cx, "history_title"),
            self.snapshots.len(),
            actions,
            cx,
        );
        let body = match (self.status, self.snapshots.is_empty()) {
            (Status::Failed, _) => {
                vec![panel_placeholder(t(cx, "history_load_failed"), cx).into_any_element()]
            }
            (Status::Loading, true) => {
                vec![panel_placeholder(t(cx, "history_loading"), cx).into_any_element()]
            }
            (_, true) => vec![panel_placeholder(t(cx, "history_empty"), cx).into_any_element()],
            (_, false) => history_list(
                &self.snapshots,
                self.preview
                    .as_ref()
                    .map(|preview| (preview.filename.as_str(), &preview.diff)),
                self.selected,
                self.restoring,
                self.focus_handle.contains_focused(window, cx),
                cx,
            ),
        };
        v_flex()
            .id("history-panel")
            .key_context("HistoryPanel")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .bg(palette.surface(1))
            .child(header)
            .child(
                div()
                    .id("history-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .p_2()
                    .children(body),
            )
    }
}
