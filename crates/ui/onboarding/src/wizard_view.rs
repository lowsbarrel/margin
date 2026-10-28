use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnimationExt, AnyElement, App, Context, ElementId, ExternalPaths, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, Role, SharedString,
    StatefulInteractiveElement, Styled, TestSupportExt, Window, div, relative, rems,
};
use margin_i18n::t;
use margin_theme::{MONO_FONT_FAMILY, MOTION_MEDIUM, MarginTheme, TextScale};

use crate::brand::card;
use crate::picker_view::quiet_button;
use crate::step_dots::step_dots;
use crate::timeline::transition;
use crate::vault_wizard::{ExitWizard, VaultWizard, WizardExit};
use crate::wizard::WizardStep;

const STEP_SLIDE_REMS: f32 = 1.2857;
const MNEMONIC_LINE_HEIGHT: f32 = 1.6;

fn heading(title_key: &str, body_key: &str, cx: &App) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    v_flex()
        .gap_1()
        .child(
            div()
                .text_ui_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text_primary)
                .child(t(cx, title_key)),
        )
        .child(
            div()
                .text_ui_sm()
                .line_height(relative(1.5))
                .text_color(palette.text_secondary)
                .child(t(cx, body_key)),
        )
}

fn field(
    icon: Option<IconName>,
    label_key: &str,
    control: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    v_flex()
        .mt_3()
        .gap_1p5()
        .child(
            h_flex()
                .gap_1p5()
                .text_ui_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(MarginTheme::palette(cx).text_tertiary)
                .when_some(icon, |label, icon| label.child(Icon::new(icon).size_4()))
                .child(t(cx, label_key)),
        )
        .child(control)
}

fn folder_step(wizard: &VaultWizard, cx: &Context<'_, VaultWizard>) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let (folder_text, folder_color): (SharedString, _) = match wizard.wizard().folder() {
        Some(folder) => (folder.to_owned().into(), palette.text_primary),
        None => (t(cx, "login_storage_placeholder"), palette.text_tertiary),
    };
    let (border, border_hover, drop_fill, drop_border) = (
        palette.border_secondary,
        palette.text_tertiary,
        palette.brand(8.0),
        palette.border_strong,
    );
    let picker = div()
        .id("onboarding-pick-folder")
        .role(Role::Button)
        .aria_label(t(cx, "login_storage_label"))
        .w_full()
        .px_3p5()
        .py_2p5()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(border)
        .bg(palette.surface(2))
        .hover(|style| style.border_color(border_hover))
        .drag_over::<ExternalPaths>(move |style, _, _, _| {
            style.bg(drop_fill).border_color(drop_border)
        })
        .cursor_pointer()
        .text_ui_sm()
        .text_color(folder_color)
        .child(div().truncate().child(folder_text))
        .on_click(cx.listener(|_, _, window, cx| VaultWizard::pick_folder(window, cx)))
        .on_drop(cx.listener(|_, paths: &ExternalPaths, window, cx| {
            VaultWizard::folder_dropped(paths, window, cx);
        }))
        .test_support();
    v_flex()
        .child(heading(
            "onboarding_wizard_folder_title",
            "onboarding_wizard_folder_body",
            cx,
        ))
        .child(field(
            Some(IconName::FolderOpen),
            "login_storage_label",
            picker,
            cx,
        ))
        .into_any_element()
}

fn name_step(wizard: &VaultWizard, cx: &App) -> AnyElement {
    v_flex()
        .child(heading(
            "onboarding_wizard_name_title",
            "onboarding_wizard_name_body",
            cx,
        ))
        .child(field(
            None,
            "login_vault_name_label",
            Input::new(wizard.name_input()).id("onboarding-vault-name"),
            cx,
        ))
        .into_any_element()
}

fn generated_words(wizard: &VaultWizard, cx: &Context<'_, VaultWizard>) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let words: SharedString = wizard
        .generated()
        .map(|mnemonic| mnemonic.expose().clone())
        .unwrap_or_default()
        .into();
    let (copy_icon, copy_rest, copy_hover) = (
        if wizard.is_copied() {
            IconName::Check
        } else {
            IconName::Copy
        },
        palette.text_tertiary,
        palette.text_primary,
    );
    let copy_label = t(cx, "login_copy_passphrase");
    v_flex()
        .gap_3()
        .child(
            div()
                .text_ui_sm()
                .italic()
                .text_color(palette.text_secondary)
                .child(t(cx, "login_generated_hint")),
        )
        .child(
            div()
                .relative()
                .p_3()
                .pr_10()
                .rounded(cx.theme().radius)
                .border_1()
                .border_dashed()
                .border_color(palette.border_secondary)
                .bg(palette.surface(2))
                .font_family(MONO_FONT_FAMILY)
                .text_ui_sm()
                .line_height(relative(MNEMONIC_LINE_HEIGHT))
                .text_color(palette.text_primary)
                .child(words)
                .child(
                    div()
                        .id("onboarding-copy-passphrase")
                        .role(Role::Button)
                        .aria_label(copy_label.clone())
                        .absolute()
                        .top_2()
                        .right_2()
                        .p_1()
                        .cursor_pointer()
                        .text_color(copy_rest)
                        .hover(|style| style.text_color(copy_hover))
                        .child(Icon::new(copy_icon))
                        .tooltip(move |window, cx| {
                            Tooltip::new(copy_label.clone()).build(window, cx)
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.copy_generated(cx)))
                        .test_support(),
                ),
        )
        .child(
            Button::new("onboarding-use-passphrase")
                .small()
                .w_full()
                .label(t(cx, "login_use_passphrase"))
                .on_click(cx.listener(|this, _, window, cx| this.use_generated(window, cx))),
        )
        .into_any_element()
}

fn passphrase_step(wizard: &VaultWizard, cx: &Context<'_, VaultWizard>) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    let (negative, divider) = (palette.text_negative, palette.border_secondary);
    let alternative = if wizard.generated().is_some() {
        generated_words(wizard, cx)
    } else {
        quiet_button(
            "onboarding-generate-passphrase",
            IconName::Plus,
            t(cx, "login_generate_new"),
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| this.generate(cx)))
        .test_support()
        .into_any_element()
    };
    v_flex()
        .child(heading(
            "onboarding_wizard_passphrase_title",
            "onboarding_wizard_passphrase_body",
            cx,
        ))
        .child(field(
            Some(IconName::KeyRound),
            "login_passphrase_label",
            Textarea::new(wizard.passphrase_input())
                .font_family(MONO_FONT_FAMILY)
                .aria_label(t(cx, "login_passphrase_label")),
            cx,
        ))
        .when_some(wizard.error(), |step, error| {
            step.child(
                div()
                    .mt_3()
                    .text_ui_sm()
                    .italic()
                    .text_color(negative)
                    .child(error),
            )
        })
        .child(div().mt_3().mb_1().h_px().bg(divider))
        .child(alternative)
        .into_any_element()
}

fn exit_link(cx: &Context<'_, VaultWizard>) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    let hover = palette.text_primary;
    let label = t(cx, "login_back_to_vaults");
    div().mb_1().child(
        h_flex()
            .id("onboarding-back-to-vaults")
            .role(Role::Button)
            .aria_label(label.clone())
            .gap_1p5()
            .text_ui_sm()
            .text_color(palette.text_secondary)
            .cursor_pointer()
            .hover(|style| style.text_color(hover))
            .child(Icon::new(IconName::ArrowLeft))
            .child(label)
            .on_click(cx.listener(|_, _, _, cx| cx.emit(ExitWizard)))
            .test_support(),
    )
}

impl Render for VaultWizard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let steps = self.wizard().steps();
        let step = match self.wizard().step() {
            WizardStep::Folder => folder_step(self, cx),
            WizardStep::Name => name_step(self, cx),
            WizardStep::Passphrase => passphrase_step(self, cx),
        };
        let busy = self.is_busy();
        let forward = if steps.is_last() {
            Button::new("onboarding-open-vault")
                .primary()
                .small()
                .loading(busy)
                .label(t(
                    cx,
                    if busy {
                        "login_opening"
                    } else {
                        "login_open_vault"
                    },
                ))
                .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx)))
        } else {
            Button::new("onboarding-wizard-next")
                .primary()
                .small()
                .icon(Icon::new(IconName::ArrowRight))
                .label(t(cx, "onboarding_next"))
                .disabled(!self.wizard().can_advance())
                .on_click(cx.listener(|this, _, window, cx| this.next(window, cx)))
        };
        card(cx)
            .when(self.exit() == WizardExit::ToVaults, |card| {
                card.child(exit_link(cx))
            })
            .child(div().relative().child(step).with_animation(
                ElementId::from(("wizard-step", steps.current())),
                transition(MOTION_MEDIUM),
                |content, progress| {
                    content
                        .left(rems(STEP_SLIDE_REMS * (1.0 - progress)))
                        .opacity(progress)
                },
            ))
            .child(
                div()
                    .mt_5()
                    .child(step_dots(steps, "onboarding-wizard-step", Self::go_to, cx)),
            )
            .child(
                h_flex()
                    .mt_4()
                    .justify_between()
                    .gap_2()
                    .child(div().when(!steps.is_first(), |back| {
                        back.child(
                            Button::new("onboarding-wizard-back")
                                .ghost()
                                .small()
                                .icon(Icon::new(IconName::ArrowLeft))
                                .label(t(cx, "onboarding_back"))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.previous(window, cx)),
                                ),
                        )
                    }))
                    .child(forward),
            )
    }
}
