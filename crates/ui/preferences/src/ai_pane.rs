use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{Icon, Sizable, h_flex, v_flex};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, px,
};
use margin_ask::{ApiFormat, Effort, LlmConfig, normalize_base_url};
use margin_i18n::{t, t_count, t_with};
use margin_settings::AppSettings;
use margin_theme::MONO_FONT_FAMILY;
use margin_workbench::{CurrentVault, run_async};

use crate::choice::Choice;
use crate::field::{CONTROL_HEIGHT, FieldError, InvalidForm, Tone, field_hint, labeled, note};
use crate::llm_form::llm_setting;

const FORMATS: [(ApiFormat, &str); 2] = [
    (ApiFormat::Openai, "settings_ai_format_openai"),
    (ApiFormat::Anthropic, "settings_ai_format_anthropic"),
];
const EFFORTS: [(Option<Effort>, &str); 4] = [
    (None, "settings_ai_effort_default"),
    (Some(Effort::Low), "settings_ai_effort_low"),
    (Some(Effort::Medium), "settings_ai_effort_medium"),
    (Some(Effort::High), "settings_ai_effort_high"),
];
const SECRET_PLACEHOLDER: &str = "••••••••";
const MODEL_MENU_HEIGHT: f32 = 240.;

type FormatSelect = SelectState<Vec<Choice<ApiFormat>>>;
type EffortSelect = SelectState<Vec<Choice<Option<Effort>>>>;

enum Models {
    Idle,
    Loading,
    Loaded(Vec<SharedString>),
    Failed(String),
}

pub(crate) struct AiPane {
    format: Entity<FormatSelect>,
    base_url: Entity<InputState>,
    api_key: Entity<InputState>,
    model: Entity<InputState>,
    effort: Entity<EffortSelect>,
    models: Models,
    base_url_error: Option<FieldError>,
    _subscriptions: [Subscription; 2],
}

impl AiPane {
    pub(crate) fn new(
        settings: &AppSettings,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let format = cx.new(|cx| SelectState::new(format_choices(cx), None, window, cx));
        let effort = cx.new(|cx| SelectState::new(effort_choices(cx), None, window, cx));
        let base_url = cx.new(|cx| InputState::new(window, cx));
        let api_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(SECRET_PLACEHOLDER)
        });
        let model = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(cx, "settings_ai_model_placeholder"))
        });
        let subscriptions = [
            cx.subscribe_in(
                &format,
                window,
                |this, _, _: &SelectEvent<_>, window, cx| {
                    let format = this.selected_format(cx);
                    this.base_url.update(cx, |input, cx| {
                        let default = normalize_base_url(format, "");
                        input.set_placeholder(default.clone(), window, cx);
                        input.set_value(default, window, cx);
                    });
                    this.models = Models::Idle;
                    cx.notify();
                },
            ),
            cx.subscribe(&base_url, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) && !matches!(this.models, Models::Idle) {
                    this.models = Models::Idle;
                    cx.notify();
                }
            }),
        ];
        let mut pane = Self {
            format,
            base_url,
            api_key,
            model,
            effort,
            models: Models::Idle,
            base_url_error: None,
            _subscriptions: subscriptions,
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
        let llm = settings.llm.as_ref();
        let format = llm.map_or(ApiFormat::Openai, |llm| llm.api_format);
        let effort = llm.and_then(|llm| llm.effort);
        self.format.update(cx, |select, cx| {
            select.set_selected_value(&format, window, cx);
        });
        self.base_url.update(cx, |input, cx| {
            input.set_placeholder(normalize_base_url(format, ""), window, cx);
        });
        self.effort.update(cx, |select, cx| {
            select.set_selected_value(&effort, window, cx);
        });
        let values = [
            (&self.base_url, llm.map(|llm| llm.base_url.as_str())),
            (&self.api_key, llm.map(|llm| llm.api_key.as_str())),
            (&self.model, llm.map(|llm| llm.model.as_str())),
        ];
        for (input, value) in values {
            input.update(cx, |input, cx| {
                input.set_value(value.unwrap_or_default(), window, cx);
            });
        }
        self.models = Models::Idle;
        self.base_url_error = None;
        cx.notify();
    }

    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "the window is reborrowed mutably inside entity update closures"
    )]
    pub(crate) fn relabel(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let placeholder = t(cx, "settings_ai_model_placeholder");
        self.model.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
        });
        let (formats, format) = (format_choices(cx), self.selected_format(cx));
        self.format.update(cx, |select, cx| {
            select.set_items(formats, window, cx);
            select.set_selected_value(&format, window, cx);
        });
        let (efforts, effort) = (effort_choices(cx), self.selected_effort(cx));
        self.effort.update(cx, |select, cx| {
            select.set_items(efforts, window, cx);
            select.set_selected_value(&effort, window, cx);
        });
    }

    pub(crate) fn apply(
        &mut self,
        settings: &mut AppSettings,
        cx: &mut Context<'_, Self>,
    ) -> Result<(), InvalidForm> {
        let result = llm_setting(self.config(cx));
        self.base_url_error = result.as_ref().err().copied();
        cx.notify();
        let Ok(llm) = result else {
            return Err(InvalidForm);
        };
        settings.llm = llm;
        Ok(())
    }

    fn config(&self, cx: &App) -> LlmConfig {
        let value = |input: &Entity<InputState>| input.read(cx).value().trim().to_owned();
        LlmConfig {
            api_format: self.selected_format(cx),
            base_url: value(&self.base_url),
            api_key: value(&self.api_key),
            model: value(&self.model),
            effort: self.selected_effort(cx),
        }
    }

    fn selected_format(&self, cx: &App) -> ApiFormat {
        self.format
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(ApiFormat::Openai)
    }

    fn selected_effort(&self, cx: &App) -> Option<Effort> {
        self.effort.read(cx).selected_value().copied().flatten()
    }

    fn load_models(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        self.models = Models::Loading;
        cx.notify();
        let config = self.config(cx);
        let ask = Arc::clone(&CurrentVault::expect(cx).ask);
        let listing = run_async(cx, async move { ask.list_models(&config).await });
        cx.spawn_in(window, async move |this, cx| {
            let result = listing.await;
            this.update(cx, |this, cx| {
                this.models = match result {
                    Ok(models) => {
                        Models::Loaded(models.into_iter().map(SharedString::from).collect())
                    }
                    Err(error) => Models::Failed(error.to_string()),
                };
                cx.notify();
            })
        })
        .detach();
    }

    fn model_input(&self, cx: &App) -> Input {
        let input = Input::new(&self.model)
            .h(CONTROL_HEIGHT)
            .font_family(MONO_FONT_FAMILY);
        let Models::Loaded(names) = &self.models else {
            return input;
        };
        let (names, model) = (names.clone(), self.model.clone());
        input.suffix(
            Button::new("model-options")
                .ghost()
                .xsmall()
                .icon(Icon::new(IconName::ChevronDown))
                .tooltip(t(cx, "settings_ai_models_pick"))
                .dropdown_menu(move |menu, _, _| {
                    names.iter().fold(
                        menu.max_h(px(MODEL_MENU_HEIGHT)).scrollable(true),
                        |menu, name| {
                            let (model, name) = (model.clone(), name.clone());
                            menu.item(PopupMenuItem::new(name.clone()).on_click(
                                move |_, window, cx| {
                                    model.update(cx, |input, cx| {
                                        input.set_value(name.clone(), window, cx);
                                    });
                                },
                            ))
                        },
                    )
                }),
        )
    }
}

impl Render for AiPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let loading = matches!(self.models, Models::Loading);
        let load_label = if loading {
            "settings_ai_loading_models"
        } else {
            "settings_ai_load_models"
        };
        let loaded = match &self.models {
            Models::Loaded(names) if !names.is_empty() => Some(note(
                t_count(cx, "settings_ai_models_loaded", names.len() as u64, &[]),
                Tone::Positive,
                cx,
            )),
            _ => None,
        };
        let failed = match &self.models {
            Models::Failed(error) => Some(note(
                t_with(
                    cx,
                    "settings_ai_models_failed",
                    &[("error", error.as_str())],
                ),
                Tone::Negative,
                cx,
            )),
            _ => None,
        };
        let base_url = Input::new(&self.base_url)
            .h(CONTROL_HEIGHT)
            .font_family(MONO_FONT_FAMILY);
        let api_key = Input::new(&self.api_key)
            .h(CONTROL_HEIGHT)
            .font_family(MONO_FONT_FAMILY);
        v_flex()
            .gap_3()
            .child(labeled(
                t(cx, "settings_ai_format"),
                Select::new(&self.format).h(CONTROL_HEIGHT),
                None,
                cx,
            ))
            .child(labeled(
                t(cx, "settings_ai_base_url"),
                base_url,
                self.base_url_error,
                cx,
            ))
            .child(field_hint(
                labeled(t(cx, "settings_ai_api_key"), api_key, None, cx),
                t(cx, "settings_ai_api_key_hint"),
                cx,
            ))
            .child(labeled(
                t(cx, "settings_ai_model"),
                self.model_input(cx),
                None,
                cx,
            ))
            .child(field_hint(
                labeled(
                    t(cx, "settings_ai_effort"),
                    Select::new(&self.effort).h(CONTROL_HEIGHT),
                    None,
                    cx,
                ),
                t(cx, "settings_ai_effort_hint"),
                cx,
            ))
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("load-models")
                            .outline()
                            .h(CONTROL_HEIGHT)
                            .icon(Icon::new(IconName::Download))
                            .label(t(cx, load_label))
                            .loading(loading)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if !matches!(this.models, Models::Loading) {
                                    this.load_models(window, cx);
                                }
                            })),
                    )
                    .children(loaded),
            )
            .children(failed)
            .child(note(t(cx, "settings_ai_privacy"), Tone::Neutral, cx))
    }
}

fn format_choices(cx: &App) -> Vec<Choice<ApiFormat>> {
    FORMATS
        .iter()
        .map(|(format, key)| Choice::new(t(cx, key), *format))
        .collect()
}

fn effort_choices(cx: &App) -> Vec<Choice<Option<Effort>>> {
    EFFORTS
        .iter()
        .map(|(effort, key)| Choice::new(t(cx, key), *effort))
        .collect()
}
