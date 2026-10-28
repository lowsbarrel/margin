#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "crate-private state shared between the canvas modules"
)]

use std::mem;
use std::rc::Rc;

use crate::camera::Camera;
use crate::gesture::{Gesture, PressButton, PressOutcome};
use crate::history::UndoHistory;
use crate::model::{
    Drawing, INK_COLOR, Shape, ShapeKind, Stroke, StrokeTool, TextLabel, WorldPoint,
};
use crate::snapping;
use crate::tool::{Tool, ToolSizes};

const MIN_SHAPE_EXTENT: f64 = 2.0;
const ERASER_COLOR: &str = "#000";

#[derive(Debug)]
pub(crate) struct Board {
    pub(crate) drawing: Rc<Drawing>,
    pub(crate) camera: Camera,
    pub(crate) tool: Tool,
    pub(crate) last_shape: ShapeKind,
    pub(crate) color: String,
    pub(crate) sizes: ToolSizes,
    pub(crate) space_held: bool,
    pub(crate) gesture: Gesture,
    pub(crate) label_widths: Vec<f64>,
    pub(crate) revision: u64,
    history: UndoHistory,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            drawing: Rc::default(),
            camera: Camera::default(),
            tool: Tool::Pen,
            last_shape: ShapeKind::Rect,
            color: INK_COLOR.to_owned(),
            sizes: ToolSizes::default(),
            space_held: false,
            gesture: Gesture::Idle,
            label_widths: Vec::new(),
            revision: 0,
            history: UndoHistory::default(),
        }
    }
}

impl Board {
    pub(crate) fn load(&mut self, drawing: Drawing, camera: Camera) {
        self.drawing = Rc::new(drawing);
        self.camera = camera;
        self.gesture = Gesture::Idle;
        self.history.clear();
    }

    pub(crate) const fn is_panning(&self) -> bool {
        matches!(self.gesture, Gesture::Pan { .. })
    }

    pub(crate) const fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        if let Tool::Shape(kind) = tool {
            self.last_shape = kind;
        }
    }

    const fn touch(&mut self) {
        self.revision += 1;
    }

    fn edit(&mut self) -> &mut Drawing {
        self.history.record(&self.drawing);
        self.touch();
        Rc::make_mut(&mut self.drawing)
    }

    fn snap(&self, point: WorldPoint) -> Option<WorldPoint> {
        let points = snapping::snap_points(
            &self.drawing.shapes,
            &self.drawing.labels,
            &self.label_widths,
        );
        snapping::find_nearest(&points, point, self.camera.zoom)
    }

    pub(crate) fn press(&mut self, local: (f64, f64), button: PressButton) -> PressOutcome {
        let point = self.camera.to_world(local.0, local.1);
        if button == PressButton::Middle || self.space_held || self.tool == Tool::Hand {
            self.gesture = Gesture::Pan {
                grab: local,
                start: self.camera,
            };
            return PressOutcome::Handled;
        }
        let size = self.sizes.current(self.tool);
        self.gesture = match self.tool {
            Tool::Hand => return PressOutcome::Handled,
            Tool::Text => return PressOutcome::EditText(point),
            Tool::Pen | Tool::Eraser => {
                let eraser = self.tool == Tool::Eraser;
                Gesture::Draw(Stroke {
                    points: vec![point],
                    color: if eraser {
                        ERASER_COLOR
                    } else {
                        self.color.as_str()
                    }
                    .to_owned(),
                    size,
                    tool: if eraser {
                        StrokeTool::Eraser
                    } else {
                        StrokeTool::Pen
                    },
                })
            }
            Tool::Shape(kind) => {
                let snap_start = if kind.is_segment() {
                    self.snap(point)
                } else {
                    None
                };
                let start = snap_start.unwrap_or(point);
                Gesture::Shape {
                    shape: Shape {
                        kind,
                        x1: start.x,
                        y1: start.y,
                        x2: start.x,
                        y2: start.y,
                        color: self.color.clone(),
                        size,
                    },
                    snap_start,
                    snap_end: None,
                }
            }
        };
        PressOutcome::Handled
    }

    pub(crate) fn drag(&mut self, local: (f64, f64)) {
        let point = self.camera.to_world(local.0, local.1);
        let mut gesture = mem::replace(&mut self.gesture, Gesture::Idle);
        match &mut gesture {
            Gesture::Idle => {}
            Gesture::Pan { grab, start } => {
                self.camera.x = start.x - (local.0 - grab.0) / self.camera.zoom;
                self.camera.y = start.y - (local.1 - grab.1) / self.camera.zoom;
            }
            Gesture::Draw(stroke) => stroke.points.push(point),
            Gesture::Shape {
                shape, snap_end, ..
            } => {
                *snap_end = if shape.kind.is_segment() {
                    self.snap(point)
                } else {
                    None
                };
                let end = snap_end.unwrap_or(point);
                shape.x2 = end.x;
                shape.y2 = end.y;
            }
        }
        self.gesture = gesture;
    }

    pub(crate) fn release(&mut self) {
        match mem::replace(&mut self.gesture, Gesture::Idle) {
            Gesture::Pan { .. } => self.touch(),
            Gesture::Draw(stroke) if stroke.points.len() >= 2 => self.edit().strokes.push(stroke),
            Gesture::Shape { shape, .. }
                if (shape.x2 - shape.x1).abs() > MIN_SHAPE_EXTENT
                    || (shape.y2 - shape.y1).abs() > MIN_SHAPE_EXTENT =>
            {
                self.edit().shapes.push(shape);
            }
            _ => {}
        }
    }

    pub(crate) fn add_label(&mut self, at: WorldPoint, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let label = TextLabel {
            x: at.x,
            y: at.y,
            text: text.to_owned(),
            color: self.color.clone(),
            font_size: self.sizes.current(Tool::Text),
        };
        self.edit().labels.push(label);
    }

    pub(crate) fn clear(&mut self) {
        if !self.drawing.is_empty() {
            *self.edit() = Drawing::default();
        }
    }

    pub(crate) fn undo(&mut self) {
        if self.history.undo(&mut self.drawing) {
            self.touch();
        }
    }

    pub(crate) fn redo(&mut self) {
        if self.history.redo(&mut self.drawing) {
            self.touch();
        }
    }

    pub(crate) fn pan_by(&mut self, delta_x: f64, delta_y: f64) {
        self.camera.pan_by(delta_x, delta_y);
        self.touch();
    }

    pub(crate) fn zoom_at(&mut self, local: (f64, f64), factor: f64) {
        self.camera.zoom_at(local.0, local.1, factor);
        self.touch();
    }

    pub(crate) fn reset_view(&mut self) {
        self.camera = Camera::default();
        self.touch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(board: &mut Board, points: &[(f64, f64)]) {
        board.press(points[0], PressButton::Primary);
        for point in &points[1..] {
            board.drag(*point);
        }
        board.release();
    }

    #[test]
    fn line_snaps_to_existing_rect_corner() {
        let mut board = Board::default();
        board.set_tool(Tool::Shape(ShapeKind::Rect));
        stroke(&mut board, &[(0.0, 0.0), (100.0, 100.0)]);
        board.set_tool(Tool::Shape(ShapeKind::Line));
        stroke(&mut board, &[(300.0, 300.0), (96.0, 103.0)]);
        let line = &board.drawing.shapes[1];
        assert_eq!((line.x2, line.y2), (100.0, 100.0));
    }

    #[test]
    fn eraser_records_an_eraser_stroke_and_keeps_everything_else() {
        let mut board = Board::default();
        stroke(&mut board, &[(0.0, 0.0), (100.0, 0.0)]);
        board.set_tool(Tool::Shape(ShapeKind::Rect));
        stroke(&mut board, &[(0.0, -10.0), (100.0, 10.0)]);
        board.set_tool(Tool::Eraser);
        stroke(&mut board, &[(50.0, -20.0), (50.0, 20.0)]);
        assert_eq!(board.drawing.shapes.len(), 1);
        let erased = &board.drawing.strokes[1];
        assert_eq!(erased.tool, StrokeTool::Eraser);
        assert_eq!(erased.color, ERASER_COLOR);
        assert!((erased.size - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hand_pans_even_over_drawn_items() {
        let mut board = Board::default();
        board.add_label(WorldPoint::new(10.0, 10.0), "  hi ");
        assert_eq!(board.drawing.labels[0].text, "hi");
        board.set_tool(Tool::Hand);
        stroke(&mut board, &[(12.0, 12.0), (2.0, -8.0)]);
        assert_eq!((board.camera.x, board.camera.y), (10.0, 20.0));
        assert!((board.drawing.labels[0].x - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn single_click_strokes_and_tiny_shapes_are_dropped() {
        let mut board = Board::default();
        stroke(&mut board, &[(5.0, 5.0)]);
        board.set_tool(Tool::Shape(ShapeKind::Ellipse));
        stroke(&mut board, &[(5.0, 5.0), (6.0, 7.0)]);
        board.add_label(WorldPoint::new(0.0, 0.0), "   ");
        assert!(board.drawing.is_empty());
        assert_eq!(board.revision, 0);
    }

    #[test]
    fn clear_is_one_undo_step() {
        let mut board = Board::default();
        stroke(&mut board, &[(0.0, 0.0), (10.0, 10.0)]);
        board.add_label(WorldPoint::new(0.0, 0.0), "a");
        board.clear();
        assert!(board.drawing.is_empty());
        board.undo();
        assert_eq!(board.drawing.strokes.len(), 1);
        assert_eq!(board.drawing.labels.len(), 1);
    }
}
