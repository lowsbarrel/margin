use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, px,
};
use margin_i18n::t;
use margin_kernel::Secret;
use margin_theme::{MONO_FONT_FAMILY, MarginTheme, TextScale};
use margin_workbench::{AppServices, CurrentVault, notify_error, notify_info, run_blocking};

use crate::field::{CONTROL_HEIGHT, labeled};

const MASKED_PASSPHRASE: &str = "••••••••••••••••••••••••";
const FALLBACK_NAME: &str = "Vault";

pub(crate) struct VaultPane {
    name: Entity<InputState>,
    passphrase: Option<Secret<String>>,
}

impl VaultPane {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let current = CurrentVault::expect(cx).name.clone();
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t(cx, "login_vault_name_placeholder"))
                .default_value(current)
        });
        Self {
            name,
            passphrase: None,
        }
    }

    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "the window is reborrowed mutably inside entity update closures"
    )]
    pub(crate) fn relabel(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let placeholder = t(cx, "login_vault_name_placeholder");
        self.name.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
        });
    }

    fn rename(&self, window: &Window, cx: &Context<'_, Self>) {
        let name = profile_name(&self.name.read(cx).value());
        let vault_path = CurrentVault::expect(cx).root.to_string_lossy().into_owned();
        let vaults = Arc::clone(&cx.global::<AppServices>().vaults);
        let rename = run_blocking(cx, move || vaults.rename_profile(&vault_path, &name));
        cx.spawn_in(window, async move |_, cx| {
            let result = rename.await;
            cx.update(|window, cx| match result {
                Ok(()) => notify_info(window, cx, "toast_settings_saved"),
                Err(error) => notify_error(window, cx, "toast_vault_rename_failed", &error),
            })
        })
        .detach();
    }

    fn toggle_passphrase(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        if self.passphrase.take().is_some() {
            cx.notify();
            return;
        }
        let vault_path = CurrentVault::expect(cx).root.to_string_lossy().into_owned();
        let vaults = Arc::clone(&cx.global::<AppServices>().vaults);
        let reveal = run_blocking(cx, move || vaults.reveal_mnemonic(&vault_path));
        cx.spawn_in(window, async move |this, cx| {
            let result = reveal.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(passphrase) => {
                    this.passphrase = Some(passphrase);
                    cx.notify();
                }
                Err(error) => notify_error(window, cx, "toast_passphrase_reveal_failed", &error),
            })
        })
        .detach();
    }
}

impl Render for VaultPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let readout = || {
            h_flex()
                .gap_2()
                .px_3()
                .py_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(palette.border_secondary)
                .bg(palette.surface(2))
        };
        let (secret, toggle_icon, toggle_label) = match &self.passphrase {
            Some(words) => (
                div()
                    .text_color(palette.text_primary)
                    .line_height(px(22.4))
                    .child(SharedString::from(words.expose().replace(' ', "\u{2002}"))),
                IconName::EyeOff,
                "settings_vault_hide",
            ),
            None => (
                div()
                    .text_color(palette.text_tertiary)
                    .child(MASKED_PASSPHRASE),
                IconName::Eye,
                "settings_vault_show",
            ),
        };
        let root = CurrentVault::expect(cx).root.display().to_string();
        v_flex()
            .gap_3()
            .child(labeled(
                t(cx, "settings_vault_name"),
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.name).h(CONTROL_HEIGHT)),
                    )
                    .child(
                        Button::new("rename-vault")
                            .outline()
                            .h(CONTROL_HEIGHT)
                            .label(t(cx, "settings_save_short"))
                            .on_click(cx.listener(|this, _, window, cx| this.rename(window, cx))),
                    ),
                None,
                cx,
            ))
            .child(labeled(
                t(cx, "settings_vault_passphrase"),
                readout()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .font_family(MONO_FONT_FAMILY)
                            .text_ui_sm()
                            .child(secret),
                    )
                    .child(
                        Button::new("toggle-passphrase")
                            .ghost()
                            .size_7()
                            .icon(Icon::new(toggle_icon))
                            .tooltip(t(cx, toggle_label))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_passphrase(window, cx);
                            })),
                    ),
                None,
                cx,
            ))
            .child(labeled(
                t(cx, "settings_vault_location"),
                readout()
                    .text_color(palette.text_tertiary)
                    .child(Icon::new(IconName::FolderOpen).small())
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_family(MONO_FONT_FAMILY)
                            .text_ui_sm()
                            .text_color(palette.text_primary)
                            .child(root),
                    ),
                None,
                cx,
            ))
    }
}

fn profile_name(entered: &str) -> String {
    let trimmed = entered.trim();
    if trimmed.is_empty() {
        FALLBACK_NAME.to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_name_falls_back_to_vault() {
        assert_eq!(profile_name("   "), "Vault");
        assert_eq!(profile_name("  Notes "), "Notes");
    }
}
