#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "crate-private state shared between the canvas modules"
)]

pub(crate) const INK_COLOR: &str = "ink";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct WorldPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

impl WorldPoint {
    pub(crate) const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub(crate) fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StrokeTool {
    Pen,
    Eraser,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stroke {
    pub(crate) points: Vec<WorldPoint>,
    pub(crate) color: String,
    pub(crate) size: f64,
    pub(crate) tool: StrokeTool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShapeKind {
    Rect,
    Ellipse,
    Line,
    Arrow,
}

impl ShapeKind {
    pub(crate) const ALL: [Self; 4] = [Self::Rect, Self::Ellipse, Self::Line, Self::Arrow];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Line => "line",
            Self::Arrow => "arrow",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    pub(crate) const fn is_segment(self) -> bool {
        matches!(self, Self::Line | Self::Arrow)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shape {
    pub(crate) kind: ShapeKind,
    pub(crate) x1: f64,
    pub(crate) y1: f64,
    pub(crate) x2: f64,
    pub(crate) y2: f64,
    pub(crate) color: String,
    pub(crate) size: f64,
}

impl Shape {
    pub(crate) const fn start(&self) -> WorldPoint {
        WorldPoint::new(self.x1, self.y1)
    }

    pub(crate) const fn end(&self) -> WorldPoint {
        WorldPoint::new(self.x2, self.y2)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextLabel {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) text: String,
    pub(crate) color: String,
    pub(crate) font_size: f64,
}

impl TextLabel {
    pub(crate) fn line_height(&self) -> f64 {
        self.font_size * 1.2
    }

    pub(crate) fn estimated_width(&self) -> f64 {
        f64::from(u32::try_from(self.text.chars().count()).unwrap_or(u32::MAX))
            * self.font_size
            * 0.6
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Drawing {
    pub(crate) strokes: Vec<Stroke>,
    pub(crate) shapes: Vec<Shape>,
    pub(crate) labels: Vec<TextLabel>,
}

impl Drawing {
    pub(crate) fn is_empty(&self) -> bool {
        self.strokes.is_empty() && self.shapes.is_empty() && self.labels.is_empty()
    }
}
