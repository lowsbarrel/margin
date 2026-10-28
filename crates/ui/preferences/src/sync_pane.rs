use std::array;

use gpui_kit::assets::IconName;
use gpui_kit::base::TestSupportExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, AppContext, ClickEvent, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window,
};
use margin_i18n::{t, t_count, t_with};
use margin_settings::{AppSettings, S3Config};
use margin_sync::ConflictStrategy;
use margin_theme::{MONO_FONT_FAMILY, MarginTheme, TextScale};
use margin_workbench::{CurrentVault, run_async};

use crate::choice::Choice;
use crate::field::{CONTROL_HEIGHT, InvalidForm, Tone, labeled, note, required};
use crate::s3_form::{DEFAULT_REGION, S3Errors, conflict_setting, conflict_strategy, s3_setting};

const SECRET_PLACEHOLDER: &str = "••••••••";

const LABELS: [&str; 5] = [
    "settings_endpoint",
    "settings_bucket",
    "settings_region",
    "settings_access_key",
    "settings_secret_key",
];
const PLACEHOLDERS: [Option<&str>; 5] = [
    Some("settings_endpoint_placeholder"),
    Some("settings_bucket_placeholder"),
    Some("settings_region_placeholder"),
    Some("settings_access_key_placeholder"),
    None,
];
const SECRET: usize = 4;
const STRATEGIES: [(ConflictStrategy, &str); 2] = [
    (ConflictStrategy::LocalWins, "settings_conflict_local_wins"),
    (ConflictStrategy::KeepNewer, "settings_conflict_keep_newer"),
];

type StrategySelect = SelectState<Vec<Choice<ConflictStrategy>>>;

pub(crate) struct SyncRequested;

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestState {
    Idle,
    Testing,
    Passed(usize),
    Failed(String),
}

pub(crate) struct SyncPane {
    inputs: [Entity<InputState>; 5],
    auto_sync: bool,
    strategy: Entity<StrategySelect>,
    errors: S3Errors,
    test: TestState,
    _strategy_changes: Subscription,
}

impl SyncPane {
    pub(crate) fn new(
        settings: &AppSettings,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let inputs = array::from_fn(|ix| {
            cx.new(|cx| {
                let input = InputState::new(window, cx).masked(ix == SECRET);
                match PLACEHOLDERS[ix] {
                    Some(key) => input.placeholder(t(cx, key)),
                    None => input.placeholder(SECRET_PLACEHOLDER),
                }
            })
        });
        let strategy = cx.new(|cx| SelectState::new(strategy_choices(cx), None, window, cx));
        let strategy_changes = cx.subscribe(&strategy, |_, _, _: &SelectEvent<_>, cx| cx.notify());
        let mut pane = Self {
            inputs,
            auto_sync: false,
            strategy,
            errors: S3Errors::default(),
            test: TestState::Idle,
            _strategy_changes: strategy_changes,
        };
        pane.load(settings, window, cx);
        pane
    }

    pub(crate) fn load(
        &mut self,
        settings: &AppSettings,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let values = settings
            .s3
            .as_ref()
            .map_or(["", "", DEFAULT_REGION, "", ""], |s3| {
                [
                    s3.endpoint.as_str(),
                    s3.bucket.as_str(),
                    s3.region.as_str(),
                    s3.access_key.as_str(),
                    s3.secret_key.as_str(),
                ]
            });
        for (input, value) in self.inputs.iter().zip(values) {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        self.auto_sync = settings.auto_sync.unwrap_or(false);
        let strategy = conflict_strategy(settings.conflict_strategy.as_deref());
        self.strategy.update(cx, |select, cx| {
            select.set_selected_value(&strategy, window, cx);
        });
        self.errors = S3Errors::default();
        self.test = TestState::Idle;
        cx.notify();
    }

    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "the window is reborrowed mutably inside entity update closures"
    )]
    pub(crate) fn relabel(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        for (input, key) in self.inputs.iter().zip(PLACEHOLDERS) {
            if let Some(key) = key {
                let placeholder = t(cx, key);
                input.update(cx, |input, cx| {
                    input.set_placeholder(placeholder, window, cx);
                });
            }
        }
        let choices = strategy_choices(cx);
        let strategy = self.selected_strategy(cx);
        self.strategy.update(cx, |select, cx| {
            select.set_items(choices, window, cx);
            select.set_selected_value(&strategy, window, cx);
        });
    }

    pub(crate) fn apply(
        &mut self,
        settings: &mut AppSettings,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), InvalidForm> {
        let result = s3_setting(self.config(cx));
        self.errors = result.as_ref().err().copied().unwrap_or_default();
        cx.notify();
        let Ok(s3) = result else {
            return Err(InvalidForm);
        };
        settings.s3 = s3;
        if settings.auto_sync.unwrap_or(false) != self.auto_sync {
            settings.auto_sync = self.auto_sync.then_some(true);
        }
        let strategy = self.selected_strategy(cx);
        if conflict_strategy(settings.conflict_strategy.as_deref()) != strategy {
            settings.conflict_strategy = Some(conflict_setting(strategy).to_owned());
        }
        Ok(())
    }

    fn config(&self, cx: &App) -> S3Config {
        let [endpoint, bucket, region, access_key, secret_key] = self
            .inputs
            .each_ref()
            .map(|input| input.read(cx).value().trim().to_owned());
        S3Config {
            endpoint,
            bucket,
            region,
            access_key,
            secret_key,
        }
    }

    fn selected_strategy(&self, cx: &App) -> ConflictStrategy {
        self.strategy
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or_default()
    }

    fn test_connection(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        let entered = self.config(cx);
        let config = match s3_setting(entered.clone()) {
            Ok(Some(config)) => config,
            Ok(None) => {
                self.errors = [
                    &entered.endpoint,
                    &entered.bucket,
                    &entered.region,
                    &entered.access_key,
                    &entered.secret_key,
                ]
                .map(|value| required(value));
                cx.notify();
                return;
            }
            Err(errors) => {
                self.errors = errors;
                cx.notify();
                return;
            }
        };
        self.errors = S3Errors::default();
        self.test = TestState::Testing;
        cx.notify();
        let service = CurrentVault::expect(cx).sync_factory.build(&config);
        let test = run_async(cx, async move { service.test_connection().await });
        cx.spawn_in(window, async move |this, cx| {
            let result = test.await;
            this.update(cx, |this, cx| {
                this.test = match result {
                    Ok(objects) => TestState::Passed(objects),
                    Err(error) => TestState::Failed(error.to_string()),
                };
                cx.notify();
            })
        })
        .detach();
    }

    fn render_test_result(&self, cx: &App) -> Option<impl IntoElement> {
        let (text, tone) = match &self.test {
            TestState::Idle | TestState::Testing => return None,
            TestState::Passed(objects) => (
                t_count(cx, "settings_test_ok", *objects as u64, &[]),
                Tone::Positive,
            ),
            TestState::Failed(error) => (
                t_with(cx, "settings_test_failed", &[("error", error.as_str())]),
                Tone::Negative,
            ),
        };
        Some(note(text, tone, cx))
    }
}

impl Render for SyncPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let testing = self.test == TestState::Testing;
        let strategy_hint = match self.selected_strategy(cx) {
            ConflictStrategy::LocalWins => "settings_conflict_hint_local",
            ConflictStrategy::KeepNewer => "settings_conflict_hint_newer",
        };
        let fields = self.inputs.iter().enumerate().map(|(ix, input)| {
            let input = Input::new(input)
                .h(CONTROL_HEIGHT)
                .font_family(MONO_FONT_FAMILY);
            labeled(t(cx, LABELS[ix]), input, self.errors[ix], cx)
        });
        let test_label = if testing {
            "settings_testing"
        } else {
            "settings_test"
        };
        let toggle_auto_sync = cx.listener(|this: &mut Self, _: &ClickEvent, _, cx| {
            this.auto_sync = !this.auto_sync;
            cx.notify();
        });
        v_flex()
            .gap_3()
            .children(fields)
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("test-connection")
                            .outline()
                            .h(CONTROL_HEIGHT)
                            .icon(Icon::new(IconName::TestTube))
                            .label(t(cx, test_label))
                            .loading(testing)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.test != TestState::Testing {
                                    this.test_connection(window, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("sync-now")
                            .success()
                            .h(CONTROL_HEIGHT)
                            .icon(Icon::new(IconName::Upload))
                            .label(t(cx, "settings_sync_now"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SyncRequested))),
                    ),
            )
            .children(self.render_test_result(cx))
            .child(
                h_flex()
                    .justify_between()
                    .py_1p5()
                    .child(
                        h_flex()
                            .id("auto-sync-label")
                            .gap_1p5()
                            .cursor_pointer()
                            .text_ui_sm()
                            .text_color(palette.text_primary)
                            .child(Icon::new(IconName::RefreshCw).small())
                            .child(t(cx, "settings_auto_sync"))
                            .on_click(toggle_auto_sync)
                            .test_support(),
                    )
                    .child(
                        Switch::new("auto-sync")
                            .checked(self.auto_sync)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.auto_sync = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.auto_sync, |this| {
                this.child(note(t(cx, "settings_auto_sync_hint"), Tone::Neutral, cx))
            })
            .child(labeled(
                t(cx, "settings_conflict_resolution"),
                Select::new(&self.strategy).h(CONTROL_HEIGHT),
                None,
                cx,
            ))
            .child(note(t(cx, strategy_hint), Tone::Neutral, cx))
    }
}

impl EventEmitter<SyncRequested> for SyncPane {}

fn strategy_choices(cx: &App) -> Vec<Choice<ConflictStrategy>> {
    STRATEGIES
        .iter()
        .map(|(strategy, key)| Choice::new(t(cx, key), *strategy))
        .collect()
}
