use crate::model::{INK_COLOR, ShapeKind};

pub(crate) const COLOR_PRESETS: [&str; 7] = [
    INK_COLOR, "#ef4444", "#f59e0b", "#22c55e", "#3b82f6", "#a855f7", "#6b7280",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Hand,
    Pen,
    Eraser,
    Shape(ShapeKind),
    Text,
}

impl Tool {
    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "h" => Self::Hand,
            "p" => Self::Pen,
            "e" => Self::Eraser,
            "r" => Self::Shape(ShapeKind::Rect),
            "o" => Self::Shape(ShapeKind::Ellipse),
            "l" => Self::Shape(ShapeKind::Line),
            "a" => Self::Shape(ShapeKind::Arrow),
            "t" => Self::Text,
            _ => return None,
        })
    }

    const fn width_values(self) -> [f64; 3] {
        match self {
            Self::Eraser => [12.0, 24.0, 48.0],
            Self::Text => [16.0, 24.0, 40.0],
            Self::Hand | Self::Pen | Self::Shape(_) => [1.5, 3.0, 6.0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StrokeWidth {
    Thin,
    Medium,
    Thick,
}

impl StrokeWidth {
    pub(crate) const ALL: [Self; 3] = [Self::Thin, Self::Medium, Self::Thick];

    pub(crate) const fn message_key(self) -> &'static str {
        match self {
            Self::Thin => "canvas_width_thin",
            Self::Medium => "canvas_width_medium",
            Self::Thick => "canvas_width_thick",
        }
    }

    pub(crate) const fn value(self, tool: Tool) -> f64 {
        tool.width_values()[self as usize]
    }

    pub(crate) const fn bar(self) -> f32 {
        match self {
            Self::Thin => 2.0,
            Self::Medium => 4.0,
            Self::Thick => 6.0,
        }
    }

    pub(crate) fn nearest(tool: Tool, size: f64) -> Self {
        let mut best = Self::Thin;
        for width in Self::ALL {
            if (width.value(tool) - size).abs() < (best.value(tool) - size).abs() {
                best = width;
            }
        }
        best
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ToolSizes {
    pen: f64,
    eraser: f64,
    text: f64,
}

impl Default for ToolSizes {
    fn default() -> Self {
        Self {
            pen: 3.0,
            eraser: 20.0,
            text: 16.0,
        }
    }
}

impl ToolSizes {
    pub(crate) const fn current(&self, tool: Tool) -> f64 {
        match tool {
            Tool::Eraser => self.eraser,
            Tool::Text => self.text,
            Tool::Hand | Tool::Pen | Tool::Shape(_) => self.pen,
        }
    }

    pub(crate) fn set(&mut self, tool: Tool, size: f64) {
        match tool {
            Tool::Eraser => self.eraser = size,
            Tool::Text => self.text = size,
            Tool::Hand | Tool::Pen | Tool::Shape(_) => self.pen = size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_width_rounds_legacy_sizes() {
        assert_eq!(
            StrokeWidth::nearest(Tool::Eraser, 20.0),
            StrokeWidth::Medium
        );
        assert_eq!(StrokeWidth::nearest(Tool::Pen, 5.0), StrokeWidth::Thick);
        assert_eq!(StrokeWidth::nearest(Tool::Text, 0.0), StrokeWidth::Thin);
    }
}
