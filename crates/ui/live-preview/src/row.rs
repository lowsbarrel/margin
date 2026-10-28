use std::ops::Range;

use margin_markdown::Span;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reveal {
    pub points: Vec<usize>,
}

impl Reveal {
    #[must_use]
    pub const fn new(points: Vec<usize>) -> Self {
        Self { points }
    }

    #[must_use]
    pub fn touches(&self, span: Span) -> bool {
        self.points.iter().any(|point| span.contains(*point))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    Line,
    Properties,
    Table,
    MathBlock { tex: String },
    Mermaid { source: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub lines: Range<usize>,
    pub span: Span,
}
