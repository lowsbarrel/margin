use crate::intro_art::Colors;
use crate::sketch::{Ink, Pose, Sketch, Spot, arc_points, frame, spot};
use crate::timeline::Timeline;

pub(crate) fn text_lines(sketch: &mut Sketch<'_>, ink: Ink, lines: &[(Spot, Spot)]) {
    for (from, to) in lines {
        sketch.line(ink, *from, *to);
    }
}

pub(crate) fn rise(progress: f32, x: f32, y: f32) -> Pose {
    Pose::STILL
        .shifted(x * (1.0 - progress), y * (1.0 - progress))
        .faded(progress)
}

pub(crate) fn files(sketch: &mut Sketch<'_>, colors: Colors, at: Timeline) {
    sketch.shape(colors.raised, colors.line, |pen| {
        pen.move_to(spot(33.0, 78.0));
        pen.corner_to(7.0, spot(40.0, 71.0));
        pen.line_to(spot(78.0, 71.0));
        pen.line_to(spot(87.0, 80.0));
        pen.line_to(spot(163.0, 80.0));
        pen.corner_to(7.0, spot(170.0, 87.0));
        pen.line_to(spot(170.0, 114.0));
        pen.corner_to(7.0, spot(163.0, 121.0));
        pen.line_to(spot(40.0, 121.0));
        pen.corner_to(7.0, spot(33.0, 114.0));
        pen.close();
    });
    for index in 0..3_u8 {
        let x = f32::from(index).mul_add(26.0, 58.0);
        let delay = f32::from(index).mul_add(170.0, 120.0);
        sketch.posed(rise(at.ease(delay, 620.0), 0.0, -34.0), |sketch| {
            sketch.rect(
                frame(x, 62.0, 34.0, 46.0, 4.0),
                colors.page,
                Some(colors.line),
            );
            text_lines(
                sketch,
                colors.line,
                &[
                    (spot(x + 7.0, 74.0), spot(x + 27.0, 74.0)),
                    (spot(x + 7.0, 82.0), spot(x + 24.0, 82.0)),
                    (spot(x + 7.0, 90.0), spot(x + 27.0, 90.0)),
                ],
            );
        });
    }
    let close = at.ease(980.0, 320.0);
    let front = Pose::STILL
        .scaled(1.0, 0.65_f32.mul_add(close, 0.35), spot(100.0, 121.0))
        .faded(close);
    sketch.posed(front, |sketch| {
        sketch.shape(colors.raised, colors.line, |pen| {
            pen.move_to(spot(33.0, 96.0));
            pen.line_to(spot(167.0, 96.0));
            pen.line_to(spot(167.0, 114.0));
            pen.corner_to(7.0, spot(160.0, 121.0));
            pen.line_to(spot(40.0, 121.0));
            pen.corner_to(7.0, spot(33.0, 114.0));
            pen.close();
        });
    });
}

pub(crate) fn passphrase(sketch: &mut Sketch<'_>, colors: Colors, at: Timeline) {
    for index in 0..12_u8 {
        let x = f32::from(index % 4).mul_add(48.0, 7.0);
        let y = f32::from(index / 4).mul_add(22.0, 14.0);
        let delay = f32::from(index).mul_add(65.0, 60.0);
        sketch.posed(rise(at.ease(delay, 380.0), 0.0, 9.0), |sketch| {
            sketch.rect(
                frame(x, y, 42.0, 16.0, 4.0),
                colors.raised,
                Some(colors.line),
            );
        });
    }
    let key = colors.brand_ink(3.0);
    let ring = arc_points(spot(72.0, 104.0), 8.0, 0.0, 360.0);
    sketch.trace(key, &ring, at.ease(900.0, 420.0));
    sketch.trace(
        key,
        &[spot(80.0, 104.0), spot(128.0, 104.0)],
        at.ease(1080.0, 420.0),
    );
    let teeth = at.ease(1240.0, 420.0) * 2.0;
    sketch.trace(key, &[spot(112.0, 104.0), spot(112.0, 96.0)], teeth);
    sketch.trace(key, &[spot(120.0, 104.0), spot(120.0, 96.0)], teeth - 1.0);
}

pub(crate) fn sync(sketch: &mut Sketch<'_>, colors: Colors, at: Timeline) {
    sketch.rect(
        frame(22.0, 42.0, 46.0, 54.0, 6.0),
        colors.raised,
        Some(colors.line),
    );
    text_lines(
        sketch,
        colors.line,
        &[
            (spot(32.0, 58.0), spot(58.0, 58.0)),
            (spot(32.0, 68.0), spot(52.0, 68.0)),
            (spot(32.0, 78.0), spot(58.0, 78.0)),
        ],
    );
    sketch.shape(colors.raised, colors.line, |pen| {
        pen.move_to(spot(136.0, 52.0));
        pen.line_to(spot(136.0, 90.0));
        pen.curve_to(spot(136.0, 94.0), spot(146.7, 97.0), spot(160.0, 97.0));
        pen.curve_to(spot(173.3, 97.0), spot(184.0, 94.0), spot(184.0, 90.0));
        pen.line_to(spot(184.0, 52.0));
    });
    sketch.shape(colors.raised, colors.line, |pen| {
        pen.ellipse(spot(160.0, 52.0), 24.0, 7.0);
    });
    let arrow = colors.muted_ink();
    sketch.trace(
        arrow,
        &[spot(74.0, 72.0), spot(124.0, 72.0)],
        at.ease(0.0, 420.0),
    );
    sketch.trace(
        arrow,
        &[spot(130.0, 65.0), spot(139.0, 72.0), spot(130.0, 79.0)],
        at.ease(360.0, 420.0),
    );
    let lock = colors.brand_ink(2.5);
    let seal = at.spring(700.0, 420.0);
    let pose = Pose::STILL
        .shifted(0.0, -26.0 * (1.0 - seal))
        .faded(seal.min(1.0));
    let shackle_progress = at.ease(980.0, 380.0);
    sketch.posed(pose, |sketch| {
        sketch.rect(frame(88.0, 66.0, 22.0, 20.0, 3.0), colors.page, Some(lock));
        let mut shackle = vec![spot(93.0, 66.0)];
        shackle.extend(arc_points(spot(99.0, 59.0), 6.0, 180.0, 360.0));
        shackle.push(spot(105.0, 66.0));
        sketch.trace(lock, &shackle, shackle_progress);
    });
}
