use std::cell::Cell;

use gpui_kit::component::WindowExt;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{App, AppContext, Focusable, ParentElement, WeakEntity, Window};
use margin_i18n::{t, t_with};
use margin_vaults::VaultProfileInfo;

use crate::vault_picker::VaultPicker;

pub(crate) fn rename(
    profile: &VaultProfileInfo,
    picker: WeakEntity<VaultPicker>,
    window: &mut Window,
    cx: &mut App,
) {
    let input = cx.new(|cx| InputState::new(window, cx).default_value(profile.name.clone()));
    let title = t(cx, "login_rename_vault_title");
    let confirm = t(cx, "login_rename_vault_confirm");
    let cancel = t(cx, "login_cancel");
    let folder = profile.vault_path.clone();
    let initial_focus = Cell::new(true);
    window.open_alert_dialog(cx, move |alert, window, cx| {
        if initial_focus.replace(false) {
            input.focus_handle(cx).focus(window, cx);
        }
        let (input_on_ok, picker, folder) = (input.clone(), picker.clone(), folder.clone());
        alert
            .title(title.clone())
            .child(Input::new(&input).id("vault-rename"))
            .ok_text(confirm.clone())
            .cancel_text(cancel.clone())
            .confirm()
            .on_ok(move |_, window, cx| {
                let name = input_on_ok.read(cx).value().trim().to_owned();
                if name.is_empty() {
                    return false;
                }
                picker
                    .update(cx, |_, cx| {
                        VaultPicker::apply_rename(folder.clone(), name, window, cx);
                    })
                    .unwrap_or_default();
                true
            })
    });
}

pub(crate) fn confirm_removal(
    profile: &VaultProfileInfo,
    picker: WeakEntity<VaultPicker>,
    window: &mut Window,
    cx: &mut App,
) {
    let title = t_with(cx, "login_remove_vault_title", &[("name", &profile.name)]);
    let body = t(cx, "login_remove_vault_body");
    let confirm = t(cx, "login_remove_vault_confirm");
    let cancel = t(cx, "login_cancel");
    let folder = profile.vault_path.clone();
    window.open_alert_dialog(cx, move |alert, _, _| {
        let (picker, folder) = (picker.clone(), folder.clone());
        alert
            .title(title.clone())
            .description(body.clone())
            .ok_text(confirm.clone())
            .ok_variant(ButtonVariant::Danger)
            .cancel_text(cancel.clone())
            .confirm()
            .on_ok(move |_, window, cx| {
                picker
                    .update(cx, |_, cx| {
                        VaultPicker::apply_removal(folder.clone(), window, cx);
                    })
                    .unwrap_or_default();
                true
            })
    });
}
