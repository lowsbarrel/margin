use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable, WindowExt, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnyElement, App, Context, EventEmitter, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Task, Window, div, rems,
};
use margin_i18n::{t, t_count, t_with};
use margin_sync::{ConflictStrategy, SyncOptions, SyncProgress, SyncReport};
use margin_theme::{MarginTheme, TextScale, css_px};
use margin_workbench::{
    CurrentVault, WorkbenchEvent, notify_error, notify_info, run_async, run_blocking,
};
use parking_lot::Mutex;

const AUTO_SYNC_INTERVAL: Duration = Duration::from_mins(5);
const PROGRESS_REFRESH: Duration = Duration::from_millis(250);
const ICON_BUTTON_PX: f32 = 26.0;
const CHIP_HEIGHT_REMS: f32 = 23.5 / 14.0;

#[derive(Clone, Debug, PartialEq, Eq)]
enum SyncState {
    Idle,
    Syncing,
    Synced,
    Failed(SharedString),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
    User,
    Schedule,
}

pub struct SyncStatus {
    state: SyncState,
    changed_during_sync: bool,
    progress: Arc<Mutex<Option<SyncProgress>>>,
    run: Task<()>,
    progress_refresh: Task<()>,
    _check: Task<()>,
    _schedule: Task<()>,
}

impl SyncStatus {
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "every status item is built through the Shell's window-and-context constructor"
    )]
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let check = CurrentVault::expect(cx).sync_service().map_or_else(
            || Task::ready(()),
            |service| {
                let pending = run_blocking(cx, move || service.has_unsynced_changes());
                cx.spawn(async move |this, cx| {
                    let pending = pending.await;
                    let _released = this.update(cx, |status, cx| {
                        if status.state == SyncState::Idle && matches!(pending, Ok(false)) {
                            status.state = SyncState::Synced;
                            cx.notify();
                        }
                    });
                })
            },
        );
        let schedule = cx.spawn_in(window, async move |this, cx| {
            while this.update_in(cx, Self::sync_on_schedule).is_ok() {
                cx.background_executor().timer(AUTO_SYNC_INTERVAL).await;
            }
        });
        Self {
            state: SyncState::Idle,
            changed_during_sync: false,
            progress: Arc::new(Mutex::new(None)),
            run: Task::ready(()),
            progress_refresh: Task::ready(()),
            _check: check,
            _schedule: schedule,
        }
    }

    pub fn sync_now(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.start(Trigger::User, window, cx);
    }

    #[must_use]
    pub fn is_syncing(&self) -> bool {
        self.state == SyncState::Syncing
    }

    pub fn mark_local_change(&mut self, cx: &mut Context<'_, Self>) {
        match self.state {
            SyncState::Synced => {
                self.state = SyncState::Idle;
                cx.notify();
            }
            SyncState::Syncing => self.changed_during_sync = true,
            SyncState::Idle | SyncState::Failed(_) => {}
        }
    }

    fn sync_on_schedule(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let enabled = CurrentVault::get(cx)
            .is_some_and(|services| services.app_settings.read().auto_sync == Some(true));
        if enabled && !self.is_syncing() {
            self.start(Trigger::Schedule, window, cx);
        }
    }

    fn start(&mut self, trigger: Trigger, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(services) = CurrentVault::get(cx) else {
            return;
        };
        let Some(service) = services.sync_service() else {
            if trigger == Trigger::User {
                notify_info(window, cx, "toast_sync_not_configured");
            }
            return;
        };
        if self.is_syncing() {
            if trigger == Trigger::User {
                notify_info(window, cx, "toast_sync_in_progress");
            }
            return;
        }
        let options = SyncOptions {
            conflict_strategy: conflict_strategy(
                services.app_settings.read().conflict_strategy.as_deref(),
            ),
        };
        self.state = SyncState::Syncing;
        self.changed_during_sync = false;
        *self.progress.lock() = None;
        let progress = Arc::clone(&self.progress);
        let work = run_async(cx, async move {
            let mut report_progress = move |update: SyncProgress| {
                *progress.lock() = Some(update);
            };
            service.sync_now(options, &mut report_progress).await
        });
        self.run = cx.spawn_in(window, async move |this, cx| {
            let outcome = work.await;
            let _released = this.update_in(cx, |status, window, cx| match outcome {
                Ok(report) => status.succeeded(report, trigger, window, cx),
                Err(error) => {
                    status.state = SyncState::Failed(SharedString::from(error.to_string()));
                    if trigger == Trigger::User {
                        notify_error(window, cx, "toast_sync_failed", &error);
                    }
                    cx.notify();
                }
            });
        });
        self.progress_refresh = cx.spawn(async move |this, cx| {
            while this
                .update(cx, |status, cx| {
                    cx.notify();
                    status.is_syncing()
                })
                .is_ok_and(|syncing| syncing)
            {
                cx.background_executor().timer(PROGRESS_REFRESH).await;
            }
        });
        cx.notify();
    }

    fn succeeded(
        &mut self,
        report: SyncReport,
        trigger: Trigger,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.state = if self.changed_during_sync {
            SyncState::Idle
        } else {
            SyncState::Synced
        };
        self.changed_during_sync = false;
        if report.downloaded + report.deleted + report.conflicts > 0 {
            cx.emit(WorkbenchEvent::TreeChanged);
        }
        if report.conflicts > 0 {
            let message = t_count(cx, "toast_sync_conflicts", report.conflicts, &[]);
            window.push_notification(Notification::info(message), cx);
        }
        if trigger == Trigger::User {
            window.push_notification(Notification::success(t(cx, "toast_sync_complete")), cx);
        }
        cx.notify();
    }

    fn chip(&self, configured: bool, cx: &App) -> AnyElement {
        let palette = MarginTheme::palette(cx);
        let state = if configured {
            &self.state
        } else {
            &SyncState::Idle
        };
        let (color, icon, label) = match state {
            SyncState::Synced => (
                palette.text_positive,
                Icon::new(IconName::Check).xsmall().into_any_element(),
                t(cx, "statusbar_synced"),
            ),
            SyncState::Syncing => (
                palette.text_brand,
                Spinner::new()
                    .icon(IconName::Loader)
                    .color(palette.text_brand)
                    .xsmall()
                    .into_any_element(),
                self.progress_label(cx),
            ),
            SyncState::Failed(_) => (
                palette.text_negative,
                Icon::new(IconName::CircleAlert).xsmall().into_any_element(),
                t(cx, "statusbar_sync_error"),
            ),
            SyncState::Idle => (
                palette.text_secondary,
                Icon::new(IconName::CloudOff).xsmall().into_any_element(),
                t(cx, "statusbar_local"),
            ),
        };
        let error = match state {
            SyncState::Failed(error) => Some(error.clone()),
            SyncState::Idle | SyncState::Syncing | SyncState::Synced => None,
        };
        h_flex()
            .id("sync-status")
            .h(rems(CHIP_HEIGHT_REMS))
            .gap_1()
            .px_2()
            .rounded_full()
            .bg(palette.surface(2))
            .text_ui_xs()
            .text_color(color)
            .child(icon)
            .child(div().whitespace_nowrap().child(label))
            .when_some(error, |chip, error| {
                chip.tooltip(move |window, cx| Tooltip::new(error.clone()).build(window, cx))
            })
            .into_any_element()
    }

    fn progress_label(&self, cx: &App) -> SharedString {
        let progress = *self.progress.lock();
        match progress {
            Some(SyncProgress { total, done }) if total > 0 => t_with(
                cx,
                "sync_status_progress",
                &[
                    ("done", done.to_string().as_str()),
                    ("total", total.to_string().as_str()),
                ],
            ),
            _ => t(cx, "statusbar_syncing"),
        }
    }
}

fn conflict_strategy(setting: Option<&str>) -> ConflictStrategy {
    match setting {
        Some("keep_newer") => ConflictStrategy::KeepNewer,
        _ => ConflictStrategy::LocalWins,
    }
}

impl fmt::Debug for SyncStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncStatus").finish_non_exhaustive()
    }
}

impl EventEmitter<WorkbenchEvent> for SyncStatus {}

impl Render for SyncStatus {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let configured =
            CurrentVault::get(cx).is_some_and(|services| services.sync_service().is_some());
        let label = t(cx, "statusbar_sync_now");
        let palette = MarginTheme::palette(cx).clone();
        h_flex().gap_1p5().child(self.chip(configured, cx)).child(
            Button::new("sync-now")
                .custom(
                    ButtonCustomVariant::new(cx)
                        .foreground(palette.text_tertiary)
                        .hover(palette.surface(2))
                        .active(palette.surface(2)),
                )
                .small()
                .size(css_px(ICON_BUTTON_PX))
                .icon(IconName::RefreshCw)
                .loading(self.is_syncing())
                .loading_icon(IconName::RefreshCw)
                .accessibility_label(label.clone())
                .tooltip(label)
                .on_click(cx.listener(|status, _, window, cx| status.sync_now(window, cx))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_keep_newer_overrides_the_local_wins_default() {
        assert_eq!(
            conflict_strategy(Some("keep_newer")),
            ConflictStrategy::KeepNewer
        );
        assert_eq!(
            conflict_strategy(Some("local_wins")),
            ConflictStrategy::LocalWins
        );
        assert_eq!(conflict_strategy(None), ConflictStrategy::LocalWins);
    }
}
