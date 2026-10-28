#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "crate-private state shared between the canvas modules"
)]

use std::f64::consts::FRAC_PI_6;
use std::rc::Rc;

use gpui_kit::{
    App, BorderStyle, Bounds, ContentMask, Hsla, PathBuilder, Pixels, Point, SharedString,
    TextAlign, TextRun, Window, font, point, px, quad, size,
};
use margin_theme::{Palette, UI_FONT_FAMILY};

use crate::camera::Camera;
use crate::model::{Drawing, Shape, ShapeKind, Stroke, StrokeTool, TextLabel, WorldPoint};
use crate::{color, erased};

const GRID_STEP: f64 = 40.0;
const SNAP_RADIUS: f32 = 4.75;
const SNAP_RING: f32 = 1.5;
const KAPPA: f32 = 0.552_284_8;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CursorRing {
    pub(crate) local: (f64, f64),
    pub(crate) diameter: f64,
    pub(crate) color: Hsla,
}

#[derive(Debug)]
pub(crate) struct Scene {
    pub(crate) drawing: Rc<Drawing>,
    pub(crate) live_stroke: Option<Stroke>,
    pub(crate) live_shape: Option<Shape>,
    pub(crate) snaps: Vec<WorldPoint>,
    pub(crate) camera: Camera,
    pub(crate) cursor: Option<CursorRing>,
    pub(crate) palette: Palette,
}

struct Painter<'scene> {
    origin: Point<Pixels>,
    camera: Camera,
    palette: &'scene Palette,
}

impl Painter<'_> {
    fn local(&self, local: (f64, f64)) -> Point<Pixels> {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "screen coordinates fit in f32"
        )]
        point(
            self.origin.x + px(local.0 as f32),
            self.origin.y + px(local.1 as f32),
        )
    }

    fn screen(&self, world: WorldPoint) -> Point<Pixels> {
        self.local(self.camera.to_local(world))
    }

    fn width(&self, world: f64) -> f32 {
        #[expect(clippy::cast_possible_truncation, reason = "stroke widths fit in f32")]
        let width = (world * self.camera.zoom) as f32;
        width
    }

    #[expect(
        clippy::unused_self,
        reason = "every painter primitive draws through the painter"
    )]
    fn dot(&self, center: Point<Pixels>, radius: f32, color: Hsla, window: &mut Window) {
        let bounds = Bounds::new(
            point(center.x - px(radius), center.y - px(radius)),
            size(px(radius * 2.0), px(radius * 2.0)),
        );
        window.paint_quad(quad(
            bounds,
            px(radius),
            color,
            px(0.0),
            color,
            BorderStyle::Solid,
        ));
    }

    #[expect(
        clippy::while_float,
        reason = "grid dots step through world coordinates"
    )]
    fn grid_dots(&self, bounds: Bounds<Pixels>) -> Vec<WorldPoint> {
        let zoom = self.camera.zoom;
        let step = GRID_STEP * (1.0 / zoom).log2().round().exp2();
        let end = self.camera.to_world(
            f64::from(bounds.size.width.as_f32()),
            f64::from(bounds.size.height.as_f32()),
        );
        let mut dots = Vec::new();
        let mut x = (self.camera.x / step).floor() * step;
        while x <= end.x {
            let mut y = (self.camera.y / step).floor() * step;
            while y <= end.y {
                dots.push(WorldPoint::new(x, y));
                y += step;
            }
            x += step;
        }
        dots
    }

    fn grid(&self, dots: &[WorldPoint], window: &mut Window) {
        for dot in dots {
            self.dot(self.screen(*dot), 1.0, self.palette.canvas_grid, window);
        }
    }

    fn round_joints(&self, joints: &[WorldPoint], width: f32, color: Hsla, window: &mut Window) {
        for joint in joints {
            self.dot(self.screen(*joint), width / 2.0, color, window);
        }
    }

    fn stroke(&self, stroke: &Stroke, window: &mut Window) {
        let [first, rest @ ..] = stroke.points.as_slice() else {
            return;
        };
        if rest.is_empty() {
            return;
        }
        let color = match stroke.tool {
            StrokeTool::Eraser => self.palette.surface(0),
            StrokeTool::Pen => color::resolve(&stroke.color, self.palette),
        };
        let width = self.width(stroke.size);
        let mut path = PathBuilder::stroke(px(width));
        path.move_to(self.screen(*first));
        for point in rest {
            path.line_to(self.screen(*point));
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
        self.round_joints(&stroke.points, width, color, window);
    }

    fn ellipse(&self, shape: &Shape, path: &mut PathBuilder) {
        let center = self.screen(WorldPoint::new(
            f64::midpoint(shape.x1, shape.x2),
            f64::midpoint(shape.y1, shape.y2),
        ));
        let radius_x = px(self.width((shape.x2 - shape.x1).abs() / 2.0));
        let radius_y = px(self.width((shape.y2 - shape.y1).abs() / 2.0));
        let at = |dx: Pixels, dy: Pixels| point(center.x + dx, center.y + dy);
        let zero = px(0.0);
        path.move_to(at(radius_x, zero));
        path.cubic_bezier_to(
            at(zero, radius_y),
            at(radius_x, radius_y * KAPPA),
            at(radius_x * KAPPA, radius_y),
        );
        path.cubic_bezier_to(
            at(-radius_x, zero),
            at(-radius_x * KAPPA, radius_y),
            at(-radius_x, radius_y * KAPPA),
        );
        path.cubic_bezier_to(
            at(zero, -radius_y),
            at(-radius_x, -radius_y * KAPPA),
            at(-radius_x * KAPPA, -radius_y),
        );
        path.cubic_bezier_to(
            at(radius_x, zero),
            at(radius_x * KAPPA, -radius_y),
            at(radius_x, -radius_y * KAPPA),
        );
        path.close();
    }

    fn shape(&self, shape: &Shape, window: &mut Window) {
        let color = color::resolve(&shape.color, self.palette);
        let width = self.width(shape.size);
        let mut path = PathBuilder::stroke(px(width));
        match shape.kind {
            ShapeKind::Rect => {
                let corners = [
                    WorldPoint::new(shape.x1, shape.y1),
                    WorldPoint::new(shape.x2, shape.y1),
                    WorldPoint::new(shape.x2, shape.y2),
                    WorldPoint::new(shape.x1, shape.y2),
                ];
                path.add_polygon(&corners.map(|corner| self.screen(corner)), true);
                self.round_joints(&corners, width, color, window);
            }
            ShapeKind::Ellipse => self.ellipse(shape, &mut path),
            ShapeKind::Line | ShapeKind::Arrow => {
                path.move_to(self.screen(shape.start()));
                path.line_to(self.screen(shape.end()));
                let mut ends = vec![shape.start(), shape.end()];
                if shape.kind == ShapeKind::Arrow {
                    let angle = (shape.y2 - shape.y1).atan2(shape.x2 - shape.x1);
                    let head = (shape.size * 3.0).max(10.0);
                    for side in [-1.0_f64, 1.0] {
                        let wing = side.mul_add(FRAC_PI_6, angle);
                        let tip = WorldPoint::new(
                            head.mul_add(-wing.cos(), shape.x2),
                            head.mul_add(-wing.sin(), shape.y2),
                        );
                        path.move_to(self.screen(shape.end()));
                        path.line_to(self.screen(tip));
                        ends.push(tip);
                    }
                }
                self.round_joints(&ends, width, color, window);
            }
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
    }

    fn label(&self, label: &TextLabel, window: &mut Window, cx: &mut App) {
        let text = SharedString::from(label.text.clone());
        let font_size = px(self.width(label.font_size));
        let run = TextRun {
            len: text.len(),
            font: font(UI_FONT_FAMILY),
            color: color::resolve(&label.color, self.palette),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(text, font_size, &[run], None);
        let height = line.ascent + line.descent;
        let mut origin = self.screen(WorldPoint::new(label.x, label.y));
        origin.y += font_size * (line.ascent / height) - line.ascent;
        let painted = line.paint(origin, height, TextAlign::Left, None, window, cx);
        drop(painted);
    }

    #[expect(
        clippy::unused_self,
        reason = "every painter primitive draws through the painter"
    )]
    fn ring(
        &self,
        center: Point<Pixels>,
        radius: f32,
        fill: Hsla,
        (border, color): (f32, Hsla),
        window: &mut Window,
    ) {
        let bounds = Bounds::new(
            point(center.x - px(radius), center.y - px(radius)),
            size(px(radius * 2.0), px(radius * 2.0)),
        );
        window.paint_quad(quad(
            bounds,
            px(radius),
            fill,
            px(border),
            color,
            BorderStyle::Solid,
        ));
    }
}

pub(crate) fn paint(scene: &Scene, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let painter = Painter {
        origin: bounds.origin,
        camera: scene.camera,
        palette: &scene.palette,
    };
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        let dots = painter.grid_dots(bounds);
        painter.grid(&dots, window);
        let strokes = || scene.drawing.strokes.iter().chain(&scene.live_stroke);
        for stroke in strokes() {
            painter.stroke(stroke, window);
        }
        painter.grid(&erased::erased_dots(strokes(), &dots), window);
        for shape in scene.drawing.shapes.iter().chain(&scene.live_shape) {
            painter.shape(shape, window);
        }
        for label in &scene.drawing.labels {
            painter.label(label, window, cx);
        }
        for snap in &scene.snaps {
            painter.ring(
                painter.screen(*snap),
                SNAP_RADIUS,
                scene.palette.brand(80.0),
                (SNAP_RING, scene.palette.surface(0)),
                window,
            );
        }
        if let Some(ring) = scene.cursor {
            #[expect(clippy::cast_possible_truncation, reason = "cursor sizes fit in f32")]
            let radius = (ring.diameter as f32 / 2.0).max(2.0);
            painter.ring(
                painter.local(ring.local),
                radius,
                gpui_kit::transparent_black(),
                (1.5, ring.color),
                window,
            );
        }
    });
}
