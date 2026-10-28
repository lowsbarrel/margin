use std::mem;

use gpui_kit::{
    BorderStyle, Bounds, Hsla, PathBuilder, Pixels, Point, Window, point, px, quad, size,
};

pub(crate) use crate::spot::{Spot, arc_points, spot};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ink {
    color: Hsla,
    width: f32,
}

impl Ink {
    pub(crate) const fn new(color: Hsla, width: f32) -> Self {
        Self { color, width }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    shift: Spot,
    scale: Spot,
    turn: f32,
    pivot: Spot,
    alpha: f32,
}

impl Pose {
    pub(crate) const STILL: Self = Self {
        shift: spot(0.0, 0.0),
        scale: spot(1.0, 1.0),
        turn: 0.0,
        pivot: spot(0.0, 0.0),
        alpha: 1.0,
    };

    pub(crate) const fn shifted(self, x: f32, y: f32) -> Self {
        Self {
            shift: spot(x, y),
            ..self
        }
    }

    pub(crate) const fn scaled(self, x: f32, y: f32, pivot: Spot) -> Self {
        Self {
            scale: spot(x, y),
            pivot,
            ..self
        }
    }

    pub(crate) const fn turned(self, degrees: f32, pivot: Spot) -> Self {
        Self {
            turn: degrees,
            pivot,
            ..self
        }
    }

    pub(crate) const fn faded(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }

    fn apply(self, at: Spot) -> Spot {
        let scaled = spot(
            (at.x - self.pivot.x).mul_add(self.scale.x, self.pivot.x),
            (at.y - self.pivot.y).mul_add(self.scale.y, self.pivot.y),
        );
        scaled
            .rotated_around(self.pivot, self.turn)
            .translated(self.shift)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Viewport {
    origin: Point<Pixels>,
    scale: f32,
}

impl Viewport {
    pub(crate) fn fit(bounds: Bounds<Pixels>, view_width: f32, view_height: f32) -> Self {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        let scale = (width / view_width).min(height / view_height);
        let offset = point(
            px(view_width.mul_add(-scale, width) / 2.0),
            px(view_height.mul_add(-scale, height) / 2.0),
        );
        Self {
            origin: bounds.origin + offset,
            scale,
        }
    }

    fn map(self, at: Spot) -> Point<Pixels> {
        self.origin + point(px(at.x * self.scale), px(at.y * self.scale))
    }

    fn length(self, value: f32) -> Pixels {
        px(value * self.scale)
    }
}

pub(crate) struct Pen {
    builder: PathBuilder,
    viewport: Viewport,
    pose: Pose,
}

impl Pen {
    fn map(&self, at: Spot) -> Point<Pixels> {
        self.viewport.map(self.pose.apply(at))
    }

    pub(crate) fn move_to(&mut self, at: Spot) {
        self.builder.move_to(self.map(at));
    }

    pub(crate) fn line_to(&mut self, at: Spot) {
        self.builder.line_to(self.map(at));
    }

    pub(crate) fn corner_to(&mut self, radius: f32, at: Spot) {
        let radii = point(
            self.viewport.length(radius * self.pose.scale.x.abs()),
            self.viewport.length(radius * self.pose.scale.y.abs()),
        );
        let to = self.map(at);
        self.builder
            .arc_to(radii, px(self.pose.turn), false, true, to);
    }

    pub(crate) fn curve_to(&mut self, control_a: Spot, control_b: Spot, at: Spot) {
        let (to, a, b) = (self.map(at), self.map(control_a), self.map(control_b));
        self.builder.cubic_bezier_to(to, a, b);
    }

    pub(crate) fn polygon(&mut self, corners: &[Spot], closed: bool) {
        let points: Vec<Point<Pixels>> = corners.iter().map(|corner| self.map(*corner)).collect();
        self.builder.add_polygon(&points, closed);
    }

    pub(crate) fn ellipse(&mut self, center: Spot, radius_x: f32, radius_y: f32) {
        let left = spot(center.x - radius_x, center.y);
        let right = spot(center.x + radius_x, center.y);
        let radii = point(
            self.viewport.length(radius_x * self.pose.scale.x.abs()),
            self.viewport.length(radius_y * self.pose.scale.y.abs()),
        );
        let (from, to) = (self.map(left), self.map(right));
        self.builder.move_to(from);
        self.builder
            .arc_to(radii, px(self.pose.turn), false, true, to);
        self.builder
            .arc_to(radii, px(self.pose.turn), false, true, from);
        self.builder.close();
    }

    pub(crate) fn round_rect(&mut self, frame: Frame) {
        let Frame {
            origin,
            width,
            height,
            radius,
        } = frame;
        let (left, top) = (origin.x, origin.y);
        let (right, bottom) = (left + width, top + height);
        self.move_to(spot(left + radius, top));
        self.line_to(spot(right - radius, top));
        self.corner_to(radius, spot(right, top + radius));
        self.line_to(spot(right, bottom - radius));
        self.corner_to(radius, spot(right - radius, bottom));
        self.line_to(spot(left + radius, bottom));
        self.corner_to(radius, spot(left, bottom - radius));
        self.line_to(spot(left, top + radius));
        self.corner_to(radius, spot(left + radius, top));
        self.close();
    }

    pub(crate) fn close(&mut self) {
        self.builder.close();
    }
}

pub(crate) struct Sketch<'window> {
    viewport: Viewport,
    pose: Pose,
    window: &'window mut Window,
}

impl<'window> Sketch<'window> {
    pub(crate) const fn new(viewport: Viewport, window: &'window mut Window) -> Self {
        Self {
            viewport,
            pose: Pose::STILL,
            window,
        }
    }

    pub(crate) fn posed(&mut self, pose: Pose, draw: impl FnOnce(&mut Self)) {
        if pose.alpha <= 0.0 {
            return;
        }
        let previous = mem::replace(&mut self.pose, pose);
        draw(self);
        self.pose = previous;
    }

    pub(crate) fn fill(&mut self, color: Hsla, draw: impl Fn(&mut Pen)) {
        self.paint(PathBuilder::fill(), color, &draw);
    }

    pub(crate) fn stroke(&mut self, ink: Ink, draw: impl Fn(&mut Pen)) {
        let builder = PathBuilder::stroke(self.viewport.length(ink.width));
        self.paint(builder, ink.color, &draw);
    }

    pub(crate) fn shape(&mut self, fill: Hsla, ink: Ink, draw: impl Fn(&mut Pen)) {
        self.fill(fill, &draw);
        self.stroke(ink, draw);
    }

    pub(crate) fn line(&mut self, ink: Ink, from: Spot, to: Spot) {
        self.stroke(ink, |pen| {
            pen.move_to(from);
            pen.line_to(to);
        });
    }

    pub(crate) fn trace(&mut self, ink: Ink, points: &[Spot], progress: f32) {
        let Some((first, rest)) = points.split_first() else {
            return;
        };
        let total: f32 = points
            .iter()
            .zip(rest)
            .map(|(from, to)| from.distance(*to))
            .sum();
        let mut budget = total * progress.clamp(0.0, 1.0);
        if budget <= 0.0 {
            return;
        }
        let mut drawn = vec![*first];
        let mut from = *first;
        for to in rest {
            let length = from.distance(*to);
            if length >= budget {
                drawn.push(from.lerp(*to, budget / length));
                break;
            }
            budget -= length;
            drawn.push(*to);
            from = *to;
        }
        self.stroke(ink, |pen| pen.polygon(&drawn, false));
    }

    pub(crate) fn rect(&mut self, frame: Frame, fill: Hsla, ink: Option<Ink>) {
        if self.pose.turn != 0.0 {
            match ink {
                Some(ink) => self.shape(fill, ink, |pen| pen.round_rect(frame)),
                None => self.fill(fill, |pen| pen.round_rect(frame)),
            }
            return;
        }
        let Pose { scale, alpha, .. } = self.pose;
        let origin = self.viewport.map(self.pose.apply(frame.origin));
        let bounds = Bounds::new(
            origin,
            size(
                self.viewport.length(frame.width * scale.x),
                self.viewport.length(frame.height * scale.y),
            ),
        );
        let fill = fill.opacity(alpha);
        let (border_width, border_color) = ink.map_or((px(0.0), fill), |ink| {
            (self.viewport.length(ink.width), ink.color.opacity(alpha))
        });
        self.window.paint_quad(quad(
            bounds,
            self.viewport.length(frame.radius * scale.x.min(scale.y)),
            fill,
            border_width,
            border_color,
            BorderStyle::default(),
        ));
    }

    fn paint(&mut self, builder: PathBuilder, color: Hsla, draw: &dyn Fn(&mut Pen)) {
        let mut pen = Pen {
            builder,
            viewport: self.viewport,
            pose: self.pose,
        };
        draw(&mut pen);
        if let Ok(path) = pen.builder.build() {
            self.window.paint_path(path, color.opacity(self.pose.alpha));
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    origin: Spot,
    width: f32,
    height: f32,
    radius: f32,
}

pub(crate) const fn frame(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Frame {
    Frame {
        origin: spot(x, y),
        width,
        height,
        radius,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_the_view_box_centred_in_the_bounds() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(400.0), px(130.0)));
        let viewport = Viewport::fit(bounds, 200.0, 130.0);
        assert_eq!(viewport.map(spot(0.0, 0.0)), point(px(100.0), px(0.0)));
        assert_eq!(
            viewport.map(spot(200.0, 130.0)),
            point(px(300.0), px(130.0))
        );
    }

    #[test]
    fn a_pose_scales_around_its_pivot_then_shifts() {
        let pose = Pose::STILL
            .scaled(1.0, 0.5, spot(0.0, 10.0))
            .shifted(2.0, 0.0);
        assert_eq!(pose.apply(spot(4.0, 0.0)), spot(6.0, 5.0));
        assert_eq!(Pose::STILL.apply(spot(3.0, 4.0)), spot(3.0, 4.0));
    }
}
