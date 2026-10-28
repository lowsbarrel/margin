use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{Sizable, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use margin_i18n::{t, t_with};
use margin_theme::{MarginTheme, TextScale};
use margin_workbench::OpenSettings;

use crate::ask_session::AskSession;
use crate::chrome::{self, icon, scaled};
use crate::citations;
use crate::spotlight::Spotlight;

const NOTE_GUTTER: f32 = 40.0;

pub(crate) fn render(ask: &AskSession, cx: &Context<'_, Spotlight>) -> AnyElement {
    if !ask.is_configured() {
        return not_configured(cx);
    }
    if ask.question().is_empty() {
        return chrome::empty_state("spotlight_ask_hint", cx);
    }
    let palette = MarginTheme::palette(cx).clone();
    let steps = (!ask.steps().is_empty()).then(|| {
        v_flex()
            .gap_1()
            .px_4()
            .py_2p5()
            .border_b_1()
            .border_color(palette.border_secondary)
            .children(ask.steps().iter().map(|step| {
                h_flex()
                    .gap_2()
                    .text_ui_xs()
                    .text_color(palette.text_tertiary)
                    .child(icon(IconName::Wrench, 12.0).flex_shrink_0())
                    .child(div().min_w_0().truncate().child(step.clone()))
            }))
    });
    let body = if ask.answer().is_empty() {
        let key = if ask.is_running() {
            Some("spotlight_ask_thinking")
        } else if ask.error().is_none() {
            Some("spotlight_ask_empty")
        } else {
            None
        };
        key.map(|key| {
            div()
                .text_ui_sm()
                .text_color(palette.text_tertiary)
                .child(t(cx, key))
                .into_any_element()
        })
    } else {
        Some(answer(ask.answer(), cx))
    };
    let error = ask.error().map(|error| {
        div()
            .mt_2()
            .text_ui_xs()
            .text_color(palette.text_negative)
            .child(t_with(cx, "spotlight_ask_error", &[("error", error)]))
    });
    let footer = ask.is_running().then(|| {
        h_flex()
            .justify_between()
            .gap_2()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(palette.border_secondary)
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_ui_xs()
                    .text_color(palette.text_tertiary)
                    .child(ask.question().to_owned()),
            )
            .child(
                Button::new("spotlight-ask-stop")
                    .outline()
                    .xsmall()
                    .label(t(cx, "spotlight_ask_cancel"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel_ask(cx))),
            )
    });
    v_flex()
        .flex_shrink_1()
        .min_h_0()
        .children(steps)
        .child(
            div()
                .id("spotlight-ask-answer")
                .flex_shrink_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_4()
                .py_3()
                .children(body)
                .children(error),
        )
        .children(footer)
        .into_any_element()
}

fn answer(markdown: &str, cx: &Context<'_, Spotlight>) -> AnyElement {
    let spotlight = cx.entity().downgrade();
    div()
        .px(scaled(NOTE_GUTTER))
        .text_ui_base()
        .text_color(MarginTheme::palette(cx).text_primary)
        .child(
            TextView::markdown(
                "spotlight-answer",
                SharedString::from(citations::link_citations(markdown)),
            )
            .selectable(true)
            .on_link_click(
                move |url: &SharedString, _: &ClickEvent, window: &mut Window, cx: &mut App| {
                    drop(spotlight.update(cx, |this, cx| this.open_citation(url, window, cx)));
                },
            ),
        )
        .into_any_element()
}

fn not_configured(cx: &Context<'_, Spotlight>) -> AnyElement {
    v_flex()
        .items_start()
        .gap_3()
        .px_4()
        .py_8()
        .child(
            div()
                .text_ui_sm()
                .text_color(MarginTheme::palette(cx).text_secondary)
                .child(t(cx, "spotlight_ask_not_configured")),
        )
        .child(
            Button::new("spotlight-ask-settings")
                .outline()
                .small()
                .icon(icon(IconName::Sparkles, 13.0))
                .label(t(cx, "spotlight_ask_open_settings"))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    window.dispatch_action(Box::new(OpenSettings), cx);
                    this.dismiss(cx);
                })),
        )
        .into_any_element()
}
