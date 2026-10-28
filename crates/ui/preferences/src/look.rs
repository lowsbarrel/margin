use gpui_kit::assets::IconName;
use gpui_kit::base::TestSupportExt;
use gpui_kit::component::select::{SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    App, AppContext, Context, Div, Entity, FontWeight, InteractiveElement, ParentElement,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use margin_i18n::{Locale, Translator, t};
use margin_theme::{Appearance, MarginTheme, TextScale};

use crate::choice::Choice;
use crate::persist::{SaveNotice, update_settings};

const APPEARANCES: [(Appearance, &str, IconName); 2] = [
    (Appearance::Dark, "appearance_dark", IconName::Moon),
    (Appearance::Light, "appearance_light", IconName::Sun),
];

pub(crate) type LocaleSelect = SelectState<Vec<Choice<Locale>>>;

pub(crate) fn set_appearance(appearance: Appearance, window: &Window, cx: &mut App) {
    MarginTheme::set_appearance(appearance, cx);
    update_settings(
        |settings| settings.theme = Some(appearance.as_setting().to_owned()),
        SaveNotice::Silent,
        window,
        cx,
    );
}

pub(crate) fn set_locale(locale: Locale, window: &Window, cx: &mut App) {
    Translator::set_locale(locale, cx);
    update_settings(
        |settings| settings.locale = Some(locale.code().to_owned()),
        SaveNotice::Silent,
        window,
        cx,
    );
}

pub(crate) fn appearance_body(cx: &App) -> Div {
    let palette = MarginTheme::palette(cx);
    let dark = palette.dark;
    let radius = cx.theme().radius;
    let segments = APPEARANCES.map(|(appearance, label_key, icon)| {
        let selected = (appearance == Appearance::Dark) == dark;
        h_flex()
            .id(label_key)
            .flex_1()
            .justify_center()
            .gap_2()
            .px_3()
            .py(px(7.))
            .rounded(radius - px(2.))
            .text_ui_sm()
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .map(|segment| {
                if selected {
                    segment
                        .bg(palette.surface(0))
                        .text_color(palette.text_primary)
                        .shadow(cx.theme().shadow_tokens().sm)
                } else {
                    segment
                        .text_color(palette.text_secondary)
                        .hover(|style| style.text_color(palette.text_primary))
                }
            })
            .child(Icon::new(icon).small())
            .child(t(cx, label_key))
            .on_click(move |_, window, cx| set_appearance(appearance, window, cx))
            .test_support()
    });
    v_flex()
        .gap_3()
        .child(
            h_flex()
                .gap_0p5()
                .p_0p5()
                .rounded(radius)
                .border_1()
                .border_color(palette.border_secondary)
                .bg(palette.surface(1))
                .children(segments),
        )
        .child(
            div()
                .text_ui_xs()
                .text_color(palette.text_tertiary)
                .child(t(cx, "appearance_hint")),
        )
}

pub(crate) fn locale_select<T: 'static>(
    window: &mut Window,
    cx: &mut Context<'_, T>,
) -> (Entity<LocaleSelect>, Subscription) {
    let choices: Vec<_> = Locale::ALL
        .iter()
        .map(|locale| Choice::new(locale.native_name().into(), *locale))
        .collect();
    let current = Translator::locale(cx);
    let select = cx.new(|cx| {
        let mut state = SelectState::new(choices, None, window, cx);
        state.set_selected_value(&current, window, cx);
        state
    });
    let changes = cx.subscribe_in(
        &select,
        window,
        |_, _, event: &SelectEvent<Vec<Choice<Locale>>>, window, cx| {
            if let SelectEvent::Confirm(Some(locale)) = event
                && *locale != Translator::locale(cx)
            {
                set_locale(*locale, window, cx);
            }
        },
    );
    (select, changes)
}
