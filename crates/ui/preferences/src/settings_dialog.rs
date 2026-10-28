use gpui_kit::component::{ActiveTheme, WindowExt};
use gpui_kit::{App, AppContext, ParentElement, Pixels, Styled, Window, px};

use crate::settings_section::SettingsSection;
use crate::settings_view::SettingsView;

const DIALOG_WIDTH: Pixels = px(560.);

pub fn open_settings_dialog(window: &mut Window, cx: &mut App) {
    open_settings_section(SettingsSection::default(), window, cx);
}

pub fn open_settings_section(section: SettingsSection, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| SettingsView::new(section, window, cx));
    window.open_dialog(cx, move |dialog, _, cx| {
        let closing = view.clone();
        dialog
            .w(DIALOG_WIDTH)
            .centered()
            .close_button(false)
            .p_0()
            .overflow_hidden()
            .rounded(cx.theme().radius_tokens().xl)
            .shadow(cx.theme().shadow_tokens().lg)
            .child(view.clone())
            .on_cancel(move |_, window, cx| {
                closing.update(cx, |view, cx| view.close_requested(window, cx))
            })
    });
}
