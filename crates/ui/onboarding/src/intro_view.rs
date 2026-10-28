use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnimationExt, AnyElement, Context, ElementId, FontWeight, IntoElement, ParentElement, Styled,
    TextAlign, div, relative, rems,
};
use margin_i18n::t;
use margin_theme::{MOTION_MEDIUM, MarginTheme, TextScale};

use crate::brand::intro_brand;
use crate::intro_art::{IntroStep, intro_art};
use crate::onboarding::Onboarding;
use crate::step_dots::step_dots;
use crate::steps::Steps;
use crate::timeline::transition;

const ART_BOX_REMS: f32 = 10.7143;
const TITLE_MIN_HEIGHT_REMS: f32 = 1.8571;
const BODY_MIN_HEIGHT_REMS: f32 = 4.2857;
const STEP_RISE_REMS: f32 = 1.1429;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntroExit {
    CreateVault,
    BackToVaults,
}

fn step_content(step: IntroStep, cx: &Context<'_, Onboarding>) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    v_flex()
        .w_full()
        .items_center()
        .gap_4()
        .child(
            h_flex()
                .h(rems(ART_BOX_REMS))
                .w_full()
                .items_center()
                .justify_center()
                .child(intro_art(step, palette)),
        )
        .child(
            div()
                .min_h(rems(TITLE_MIN_HEIGHT_REMS))
                .text_ui_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.text_primary)
                .text_align(TextAlign::Center)
                .child(t(cx, step.title_key())),
        )
        .child(
            div()
                .min_h(rems(BODY_MIN_HEIGHT_REMS))
                .text_ui_sm()
                .line_height(relative(1.5))
                .text_color(palette.text_secondary)
                .text_align(TextAlign::Center)
                .child(t(cx, step.body_key())),
        )
}

pub(crate) fn render(tour: Steps, exit: IntroExit, cx: &Context<'_, Onboarding>) -> AnyElement {
    let step = IntroStep::ALL[tour.current()];
    let next_label = match (tour.is_last(), exit) {
        (false, _) => t(cx, "onboarding_next"),
        (true, IntroExit::CreateVault) => t(cx, "onboarding_get_started"),
        (true, IntroExit::BackToVaults) => t(cx, "onboarding_back_to_vaults"),
    };
    let content = step_content(step, cx);
    v_flex()
        .w_full()
        .items_center()
        .child(div().mb_6().child(intro_brand(cx)))
        .child(div().w_full().relative().child(content).with_animation(
            ElementId::from(("intro-step", tour.current())),
            transition(MOTION_MEDIUM),
            |content, progress| {
                content
                    .top(rems(STEP_RISE_REMS * (1.0 - progress)))
                    .opacity(progress)
            },
        ))
        .child(div().mt_6().child(step_dots(
            tour,
            "onboarding-tour-step",
            Onboarding::tour_go_to,
            cx,
        )))
        .child(
            h_flex()
                .mt_5()
                .w_full()
                .justify_between()
                .gap_3()
                .child(
                    Button::new("onboarding-skip")
                        .ghost()
                        .small()
                        .label(t(cx, "onboarding_skip"))
                        .on_click(cx.listener(|this, _, window, cx| this.finish_intro(window, cx))),
                )
                .child(
                    h_flex()
                        .gap_1p5()
                        .when(!tour.is_first(), |this| {
                            this.child(
                                Button::new("onboarding-back")
                                    .ghost()
                                    .small()
                                    .icon(Icon::new(IconName::ArrowLeft))
                                    .label(t(cx, "onboarding_back"))
                                    .on_click(cx.listener(|this, _, _, cx| this.tour_previous(cx))),
                            )
                        })
                        .child(
                            Button::new("onboarding-next")
                                .primary()
                                .small()
                                .when(!tour.is_last(), |this| {
                                    this.icon(Icon::new(IconName::ArrowRight))
                                })
                                .label(next_label)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.tour_next(window, cx)),
                                ),
                        ),
                ),
        )
        .into_any_element()
}
