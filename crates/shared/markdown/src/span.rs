#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    #[must_use]
    pub const fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset <= self.end
    }

    #[must_use]
    pub const fn overlaps(&self, from: usize, to: usize) -> bool {
        self.start <= to && from <= self.end
    }

    #[must_use]
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        source
            .get(self.start..self.end)
            .expect("a span always addresses the source it was parsed from")
    }

    #[must_use]
    pub fn shifted(self, delta: isize) -> Option<Self> {
        Some(Self {
            start: self.start.checked_add_signed(delta)?,
            end: self.end.checked_add_signed(delta)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub span: Span,
    pub insert: String,
}

impl TextEdit {
    #[must_use]
    pub fn new(span: Span, insert: impl Into<String>) -> Self {
        Self {
            span,
            insert: insert.into(),
        }
    }

    #[must_use]
    pub fn applied(&self, source: &str) -> String {
        let mut out = String::with_capacity(source.len() + self.insert.len());
        out.push_str(Span::new(0, self.span.start).slice(source));
        out.push_str(&self.insert);
        out.push_str(Span::new(self.span.end, source.len()).slice(source));
        out
    }
}
