use std::sync::LazyLock;

use gpui_kit::{AnimationExt, Hsla, IntoElement, ParentElement, Rems, Styled, canvas, div};

use crate::sketch::{Ink, Pose, Sketch, Spot, Viewport, frame, spot};
use crate::timeline::{Timeline, entrance};

const VIEW_BOX: f32 = 1500.0;
const MARK_OFFSET: Spot = spot(187.107, 1124.661);
const MARK_PATH: &str = include_str!("../assets/logo_mark.path");
const OUTLINE_WIDTH: f32 = 32.0;
const CURVE_SAMPLES: u8 = 12;
const DOT_CENTER: Spot = spot(1221.5, 492.6);

#[derive(Debug, PartialEq)]
struct Outline {
    start: Spot,
    curves: Vec<[Spot; 3]>,
}

impl Outline {
    fn flattened(&self) -> Vec<Spot> {
        let mut points = vec![self.start];
        let mut from = self.start;
        for [control_a, control_b, to] in &self.curves {
            points.extend((1..=CURVE_SAMPLES).map(|sample| {
                from.bezier(
                    *control_a,
                    *control_b,
                    *to,
                    f32::from(sample) / f32::from(CURVE_SAMPLES),
                )
            }));
            from = *to;
        }
        points
    }
}

static MARK: LazyLock<Outline> =
    LazyLock::new(|| parse_outline(MARK_PATH).expect("the bundled logo path is valid SVG"));

static MARK_TRACE: LazyLock<Vec<Spot>> = LazyLock::new(|| MARK.flattened());

fn parse_outline(path: &str) -> Option<Outline> {
    let mut tokens = path.trim().trim_end_matches('Z').split_whitespace();
    if tokens.next()? != "M" {
        return None;
    }
    let start = read_spot(&mut tokens)?;
    let mut curves = Vec::new();
    while let Some(command) = tokens.next() {
        if command != "C" {
            return None;
        }
        curves.push([
            read_spot(&mut tokens)?,
            read_spot(&mut tokens)?,
            read_spot(&mut tokens)?,
        ]);
    }
    Some(Outline { start, curves })
}

fn read_spot<'path>(tokens: &mut impl Iterator<Item = &'path str>) -> Option<Spot> {
    let x = tokens.next()?.parse::<f32>().ok()?;
    let y = tokens.next()?.parse::<f32>().ok()?;
    Some(spot(x, y).translated(MARK_OFFSET))
}

fn paint_mark(sketch: &mut Sketch<'_>, color: Hsla, at: Timeline) {
    let outline = 1.0 - at.ease(1100.0, 300.0);
    sketch.posed(Pose::STILL.faded(outline), |sketch| {
        let ink = Ink::new(color, OUTLINE_WIDTH);
        sketch.trace(ink, &MARK_TRACE, at.ease(100.0, 1000.0));
    });
    sketch.posed(Pose::STILL.faded(at.ease(850.0, 450.0)), |sketch| {
        sketch.fill(color, |pen| {
            pen.move_to(MARK.start);
            for [control_a, control_b, end] in &MARK.curves {
                pen.curve_to(*control_a, *control_b, *end);
            }
            pen.close();
        });
    });
    let pop = at.spring(1150.0, 360.0);
    let dot = Pose::STILL.scaled(pop, pop, DOT_CENTER).faded(pop.min(1.0));
    sketch.posed(dot, |sketch| {
        sketch.rect(frame(1135.0, 406.0, 173.0, 173.2, 86.6), color, None);
    });
}

fn mark(color: Hsla, progress: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let mut sketch = Sketch::new(Viewport::fit(bounds, VIEW_BOX, VIEW_BOX), window);
            paint_mark(&mut sketch, color, Timeline::at(progress));
        },
    )
    .size_full()
}

pub(crate) fn logo_mark(size: Rems, color: Hsla) -> impl IntoElement {
    div().size(size).child(mark(color, 1.0))
}

pub(crate) fn animated_logo_mark(size: Rems, color: Hsla) -> impl IntoElement {
    div()
        .size(size)
        .with_animation("logo-mark", entrance(), move |logo, progress| {
            logo.child(mark(color, progress))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_bundled_mark() {
        let outline = parse_outline(MARK_PATH).expect("the bundled path parses");
        assert_eq!(outline.curves.len(), 57);
        let last = outline.curves.last().expect("the mark has curves")[2];
        assert_eq!(last, outline.start, "the outline closes on its start");
    }

    #[test]
    fn rejects_unsupported_commands() {
        assert_eq!(parse_outline("M 0 0 L 1 1"), None);
        assert_eq!(parse_outline("C 0 0"), None);
        assert_eq!(parse_outline("M 0 0 C 1 2 3"), None);
    }

    #[test]
    fn the_trace_follows_every_curve_back_to_the_start() {
        let trace = MARK.flattened();
        assert_eq!(trace.len(), 1 + 57 * usize::from(CURVE_SAMPLES));
        assert_eq!(trace.last(), Some(&MARK.start));
    }
}
