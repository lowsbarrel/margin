use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AppContext, ClipboardItem, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    div, px,
};
use margin_i18n::{Locale, Translator, t, t_with};
use margin_settings::Settings;
use margin_theme::{Appearance, MONO_FONT_FAMILY, MarginTheme, TextScale};
use margin_workbench::{CurrentVault, notify_error, notify_info};

use crate::field::{CONTROL_HEIGHT, Tone, note};
use crate::persist::{SaveNotice, activate_services, save_settings};

const COPIED_FOR: Duration = Duration::from_secs(2);
const COPY_BUTTON: f32 = 26.;

pub(crate) struct GenerateRequested;

pub(crate) struct Imported;

pub(crate) struct ExportPane {
    output: Entity<TextareaState>,
    generated: bool,
    copied: bool,
}

impl ExportPane {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        Self {
            output: cx.new(|cx| TextareaState::new(window, cx).rows(3)),
            generated: false,
            copied: false,
        }
    }

    pub(crate) fn generate(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let services = CurrentVault::expect(cx);
        let exported = Settings::export_string(&services.key, &services.app_settings.read());
        match exported {
            Ok(text) => {
                self.output
                    .update(cx, |output, cx| output.set_value(text, window, cx));
                self.generated = true;
                cx.notify();
            }
            Err(error) => notify_error(window, cx, "toast_export_failed", &error),
        }
    }

    fn copy(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let text = self.output.read(cx).value();
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        notify_info(window, cx, "toast_copied");
        self.copied = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            drop(this.update(cx, |this, cx| {
                this.copied = false;
                cx.notify();
            }));
        })
        .detach();
    }
}

impl EventEmitter<GenerateRequested> for ExportPane {}

impl Render for ExportPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let copy_icon = if self.copied {
            IconName::Check
        } else {
            IconName::Copy
        };
        v_flex()
            .gap_3()
            .child(note(t(cx, "settings_export_hint"), Tone::Neutral, cx))
            .child(
                Button::new("generate-export")
                    .outline()
                    .h(CONTROL_HEIGHT)
                    .w_full()
                    .label(t(cx, "settings_export_generate"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(GenerateRequested))),
            )
            .when(self.generated, |pane| {
                pane.child(
                    div()
                        .relative()
                        .child(
                            Textarea::new(&self.output)
                                .readonly(true)
                                .font_family(MONO_FONT_FAMILY),
                        )
                        .child(
                            div()
                                .id("copy-export")
                                .absolute()
                                .top_2()
                                .right_2()
                                .size(px(COPY_BUTTON))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(cx.theme().radius_tokens().sm)
                                .border_1()
                                .border_color(palette.border_secondary)
                                .bg(palette.surface(0))
                                .text_color(palette.text_tertiary)
                                .cursor_pointer()
                                .hover(|style| style.text_color(palette.text_primary))
                                .child(Icon::new(copy_icon).small())
                                .on_click(cx.listener(|this, _, window, cx| this.copy(window, cx))),
                        ),
                )
            })
    }
}

pub(crate) struct ImportPane {
    input: Entity<TextareaState>,
    error: Option<SharedString>,
    _changes: Subscription,
}

impl ImportPane {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder(t(cx, "settings_import_placeholder"))
        });
        let changes = cx.subscribe(&input, |this, _, _: &InputEvent, cx| {
            this.error = None;
            cx.notify();
        });
        Self {
            input,
            error: None,
            _changes: changes,
        }
    }

    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "the window is reborrowed mutably inside entity update closures"
    )]
    pub(crate) fn relabel(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let placeholder = t(cx, "settings_import_placeholder");
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
        });
    }

    fn import(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let encoded = self.input.read(cx).value().trim().to_owned();
        if encoded.is_empty() {
            return;
        }
        let services = CurrentVault::expect(cx);
        match Settings::import_string(&services.key, &encoded) {
            Ok(imported) => {
                services.app_settings.write().clone_from(&imported);
                if imported.theme.is_some() {
                    MarginTheme::set_appearance(
                        Appearance::from_setting(imported.theme.as_deref()),
                        cx,
                    );
                }
                if let Some(locale) = imported.locale.as_deref().and_then(Locale::from_tag) {
                    Translator::set_locale(locale, cx);
                }
                activate_services(window, cx);
                save_settings(SaveNotice::Imported, window, cx);
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.error = None;
                cx.emit(Imported);
            }
            Err(error) => {
                self.error = Some(t_with(
                    cx,
                    "settings_import_failed",
                    &[("error", &error.to_string())],
                ));
            }
        }
        cx.notify();
    }
}

impl EventEmitter<Imported> for ImportPane {}

impl Render for ImportPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let empty = self.input.read(cx).value().trim().is_empty();
        v_flex()
            .gap_3()
            .child(note(t(cx, "settings_import_hint"), Tone::Neutral, cx))
            .child(
                v_flex()
                    .gap_1p5()
                    .child(Textarea::new(&self.input).font_family(MONO_FONT_FAMILY))
                    .when_some(self.error.clone(), |field, error| {
                        field.child(
                            div()
                                .text_ui_xs()
                                .text_color(MarginTheme::palette(cx).text_negative)
                                .child(error),
                        )
                    }),
            )
            .child(
                Button::new("import-settings")
                    .outline()
                    .h(CONTROL_HEIGHT)
                    .w_full()
                    .icon(Icon::new(IconName::Download))
                    .label(t(cx, "settings_import_btn"))
                    .disabled(empty)
                    .on_click(cx.listener(|this, _, window, cx| this.import(window, cx))),
            )
    }
}
