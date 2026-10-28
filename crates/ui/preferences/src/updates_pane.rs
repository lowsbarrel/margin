use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{Icon, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{Context, IntoElement, ParentElement, Render, Styled, Window};
use margin_i18n::{t, t_with};
use margin_workbench::{CheckForUpdates, Updates};

use crate::field::{CONTROL_HEIGHT, Tone, note};

#[derive(Clone, Debug, PartialEq, Eq)]
enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Failed(String),
}

pub(crate) struct UpdatesPane {
    state: UpdateState,
}

impl UpdatesPane {
    pub(crate) const fn new() -> Self {
        Self {
            state: UpdateState::Idle,
        }
    }

    fn check(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        if self.state == UpdateState::Checking {
            return;
        }
        if cx.try_global::<Updates>().is_none() {
            self.state = UpdateState::UpToDate;
            cx.notify();
            return;
        }
        self.state = UpdateState::Checking;
        cx.notify();
        let check = Updates::check(cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = check.await;
            this.update_in(cx, |this, window, cx| {
                this.state = match result {
                    Ok(Some(update)) => {
                        window.dispatch_action(Box::new(CheckForUpdates), cx);
                        UpdateState::Available(update.version)
                    }
                    Ok(None) => UpdateState::UpToDate,
                    Err(error) => UpdateState::Failed(error.to_string()),
                };
                cx.notify();
            })
        })
        .detach();
    }
}

impl Render for UpdatesPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let checking = self.state == UpdateState::Checking;
        let label = if checking {
            "settings_updates_checking"
        } else {
            "settings_updates_check"
        };
        let result = match &self.state {
            UpdateState::Idle | UpdateState::Checking => None,
            UpdateState::UpToDate => Some(t(cx, "settings_updates_none")),
            UpdateState::Available(version) => Some(t_with(
                cx,
                "settings_updates_found",
                &[("version", version.as_str())],
            )),
            UpdateState::Failed(error) => Some(t_with(
                cx,
                "settings_updates_failed",
                &[("error", error.as_str())],
            )),
        };
        v_flex()
            .gap_3()
            .child(
                Button::new("check-updates")
                    .outline()
                    .h(CONTROL_HEIGHT)
                    .w_full()
                    .icon(Icon::new(IconName::RefreshCw))
                    .label(t(cx, label))
                    .loading(checking)
                    .on_click(cx.listener(|this, _, window, cx| this.check(window, cx))),
            )
            .when_some(result, |pane, text| {
                pane.child(note(text, Tone::Neutral, cx).mt_2())
            })
    }
}
