use crate::span::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkTarget {
    pub destination: String,
    pub title: Option<String>,
}

impl LinkTarget {
    #[must_use]
    pub fn new(destination: impl Into<String>, title: Option<String>) -> Self {
        Self {
            destination: destination.into(),
            title,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub heading: Option<String>,
    pub block: Option<String>,
    pub alias: Option<String>,
}

impl WikiLink {
    #[must_use]
    pub fn label(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.target)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedTarget {
    pub target: String,
    pub size: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InlineKind {
    Text,
    Emphasis,
    Strong,
    Strikethrough,
    Highlight,
    Code,
    Link(Box<LinkTarget>),
    Autolink { destination: String, email: bool },
    Image(Box<LinkTarget>),
    WikiLink(Box<WikiLink>),
    Embed(EmbedTarget),
    Math,
    Comment,
    FootnoteReference { id: String },
    Tag { name: String },
    Html { name: String, closing: bool },
    SoftBreak,
    HardBreak,
    Escape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inline {
    pub kind: InlineKind,
    pub span: Span,
    pub markers: Vec<Span>,
    pub content: Span,
    pub children: Vec<Self>,
}

impl Inline {
    #[must_use]
    pub fn text(span: Span) -> Self {
        Self {
            kind: InlineKind::Text,
            span,
            markers: Vec::new(),
            content: span,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn new(kind: InlineKind, span: Span, content: Span, markers: Vec<Span>) -> Self {
        Self {
            kind,
            span,
            markers,
            content,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }

    #[must_use]
    pub const fn is_text(&self) -> bool {
        matches!(self.kind, InlineKind::Text)
    }

    pub(crate) fn shift(&mut self, delta: isize) {
        if let Some(span) = self.span.shifted(delta) {
            self.span = span;
        }
        if let Some(span) = self.content.shifted(delta) {
            self.content = span;
        }
        for marker in &mut self.markers {
            if let Some(span) = marker.shifted(delta) {
                *marker = span;
            }
        }
        for child in &mut self.children {
            child.shift(delta);
        }
    }

    pub(crate) fn collect_text<'a>(&'a self, source: &'a str, out: &mut String) {
        if self.is_text() {
            out.push_str(self.span.slice(source));
        }
        for child in &self.children {
            child.collect_text(source, out);
        }
    }
}
