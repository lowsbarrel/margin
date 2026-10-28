use gpui_kit::component::h_flex;
use gpui_kit::{
    AnimationExt, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Role,
    StatefulInteractiveElement, Styled, Window, div, rems,
};
use margin_i18n::t_with;
use margin_theme::{MOTION_SHORT, MarginTheme};

use crate::steps::Steps;
use crate::timeline::transition;

const IDLE_WIDTH_REMS: f32 = 0.375;
const ACTIVE_WIDTH_REMS: f32 = 1.0;

pub(crate) fn step_dots<View: 'static>(
    steps: Steps,
    name: &'static str,
    go_to: fn(&mut View, usize, &mut Window, &mut Context<'_, View>),
    cx: &Context<'_, View>,
) -> impl IntoElement {
    let palette = MarginTheme::palette(cx);
    let (active, idle, hover) = (
        palette.bg_brand,
        palette.border_strong,
        palette.text_tertiary,
    );
    let total = steps.count().to_string();
    let dots: Vec<_> = (0..steps.count())
        .map(|index| {
            let number = (index + 1).to_string();
            let label = t_with(
                cx,
                "onboarding_step_of",
                &[("current", number.as_str()), ("total", total.as_str())],
            );
            let dot = div()
                .id((name, index))
                .role(Role::Button)
                .aria_label(label)
                .h_1p5()
                .rounded_full()
                .cursor_pointer()
                .on_click(cx.listener(move |view, _, window, cx| go_to(view, index, window, cx)));
            if index == steps.current() {
                dot.bg(active)
                    .with_animation(
                        ElementId::from((name, index + steps.count())),
                        transition(MOTION_SHORT),
                        |dot, progress| {
                            dot.w(rems(
                                (ACTIVE_WIDTH_REMS - IDLE_WIDTH_REMS)
                                    .mul_add(progress, IDLE_WIDTH_REMS),
                            ))
                        },
                    )
                    .into_any_element()
            } else {
                dot.w(rems(IDLE_WIDTH_REMS))
                    .bg(idle)
                    .hover(|style| style.bg(hover))
                    .into_any_element()
            }
        })
        .collect();
    h_flex()
        .justify_center()
        .items_center()
        .gap_1p5()
        .h_1p5()
        .children(dots)
}
