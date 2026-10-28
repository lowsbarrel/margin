use margin_markdown::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    #[must_use]
    pub const fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    #[must_use]
    pub const fn cursor(at: usize) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    #[must_use]
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    #[must_use]
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    #[must_use]
    pub fn span(&self) -> Span {
        Span::new(self.start(), self.end())
    }
}
