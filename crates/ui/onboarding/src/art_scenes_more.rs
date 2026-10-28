use gpui_kit::{Animation, AnimationExt, IntoElement, ParentElement, Styled, canvas, div};
use margin_theme::MOTION_MEDIUM;

use crate::art_scenes::{rise, text_lines};
use crate::intro_art::{Colors, VIEW_HEIGHT, VIEW_WIDTH};
use crate::sketch::{Ink, Pose, Sketch, Viewport, arc_points, frame, spot};
use crate::timeline::Timeline;

const CARET_PERIOD: u32 = 7;
const CARET_FRAMES_PER_SECOND: f32 = 8.0;

pub(crate) fn find(sketch: &mut Sketch<'_>, colors: Colors, at: Timeline) {
    sketch.rect(
        frame(14.0, 18.0, 172.0, 94.0, 8.0),
        colors.raised,
        Some(colors.line),
    );
    let lens = colors.muted_ink();
    sketch.stroke(lens, |pen| pen.ellipse(spot(34.0, 42.0), 7.0, 7.0));
    sketch.line(lens, spot(39.0, 47.0), spot(45.0, 53.0));
    for (index, x, width) in [(0_u8, 50.0, 34.0), (1, 90.0, 20.0)] {
        let delay = f32::from(index).mul_add(220.0, 180.0);
        sketch.posed(rise(at.ease(delay, 260.0), -8.0, 0.0), |sketch| {
            sketch.rect(
                frame(x, 38.0, width, 8.0, 4.0),
                colors.page,
                Some(colors.line),
            );
        });
    }
    let divider = Ink::new(colors.divider, 1.5);
    sketch.line(divider, spot(26.0, 58.0), spot(174.0, 58.0));
    sketch.posed(rise(at.ease(760.0, 380.0), 0.0, 7.0), |sketch| {
        sketch.rect(
            frame(26.0, 70.0, 12.0, 12.0, 3.0),
            colors.brand_strong,
            None,
        );
        text_lines(
            sketch,
            colors.line,
            &[
                (spot(48.0, 74.0), spot(150.0, 74.0)),
                (spot(48.0, 84.0), spot(112.0, 84.0)),
            ],
        );
    });
    sketch.posed(rise(at.ease(960.0, 380.0), 0.0, 7.0), |sketch| {
        sketch.rect(frame(26.0, 94.0, 12.0, 12.0, 3.0), colors.brand_soft, None);
        sketch.line(colors.line, spot(48.0, 98.0), spot(130.0, 98.0));
    });
}

pub(crate) fn caret(colors: Colors) -> impl IntoElement {
    let blink = Animation::new(MOTION_MEDIUM * CARET_PERIOD)
        .repeat()
        .with_max_fps(CARET_FRAMES_PER_SECOND);
    div()
        .absolute()
        .inset_0()
        .with_animation("intro-caret", blink, move |caret, phase| {
            let visible = phase < 0.5 || phase >= 1.0;
            caret.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        if visible {
                            let viewport = Viewport::fit(bounds, VIEW_WIDTH, VIEW_HEIGHT);
                            Sketch::new(viewport, window).line(
                                colors.brand_ink(2.0),
                                spot(116.0, 34.0),
                                spot(116.0, 50.0),
                            );
                        }
                    },
                )
                .size_full(),
            )
        })
}

pub(crate) fn history(sketch: &mut Sketch<'_>, colors: Colors, at: Timeline) {
    let pivot = spot(71.0, 108.0);
    for (index, tilt) in [(0_u8, -10.0_f32), (1, 0.0), (2, 10.0)] {
        let delay = f32::from(index).mul_add(150.0, 120.0);
        let fan = at.ease(delay, 480.0);
        sketch.posed(Pose::STILL.turned(tilt * fan, pivot).faded(fan), |sketch| {
            sketch.rect(
                frame(44.0, 40.0, 54.0, 68.0, 5.0),
                colors.raised,
                Some(colors.line),
            );
            text_lines(
                sketch,
                colors.line,
                &[
                    (spot(56.0, 56.0), spot(86.0, 56.0)),
                    (spot(56.0, 66.0), spot(80.0, 66.0)),
                ],
            );
        });
    }
    let tick = at.ease(620.0, 700.0);
    let clock_center = spot(176.0, 34.0);
    let clock = Pose::STILL
        .turned(-90.0 * (1.0 - tick), clock_center)
        .faded(tick);
    sketch.posed(clock, |sketch| {
        let ink = colors.muted_ink();
        sketch.trace(ink, &arc_points(clock_center, 11.0, 0.0, 360.0), 1.0);
        text_lines(
            sketch,
            ink,
            &[
                (clock_center, spot(176.0, 29.0)),
                (clock_center, spot(180.0, 34.0)),
            ],
        );
    });
    let bin = colors.brand_ink(2.5);
    sketch.trace(
        bin,
        &[
            spot(131.0, 66.0),
            spot(135.0, 115.0),
            spot(139.0, 118.0),
            spot(165.0, 118.0),
            spot(169.0, 115.0),
            spot(173.0, 66.0),
        ],
        at.ease(700.0, 460.0),
    );
    sketch.trace(
        bin,
        &[spot(126.0, 66.0), spot(178.0, 66.0)],
        at.ease(1020.0, 460.0),
    );
    sketch.trace(
        bin,
        &[spot(144.0, 60.0), spot(160.0, 60.0)],
        at.ease(1160.0, 460.0),
    );
}
