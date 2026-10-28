use crate::callout::{CalloutKind, Fold};
use crate::frontmatter::Frontmatter;
use crate::inline::Inline;
use crate::span::Span;
use crate::table::TableModel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadingStyle {
    Atx,
    Setext,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockKind {
    Frontmatter(Frontmatter),
    Heading {
        level: u8,
        style: HeadingStyle,
    },
    Paragraph,
    BlockQuote,
    Callout {
        kind: CalloutKind,
        fold: Fold,
        title: String,
    },
    ColonCallout {
        kind: CalloutKind,
        name: String,
        title: String,
    },
    BulletList,
    OrderedList {
        start: u64,
    },
    ListItem {
        task: Option<bool>,
        loose: bool,
    },
    CodeBlock {
        info: String,
        fenced: bool,
    },
    ThematicBreak,
    MathBlock {
        single_line: bool,
    },
    Table(TableModel),
    HtmlBlock,
    Comment,
    FootnoteDefinition {
        id: String,
    },
    LinkReference {
        label: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: BlockKind,
    pub span: Span,
    pub markers: Vec<Span>,
    pub content: Span,
    pub inlines: Vec<Inline>,
    pub children: Vec<Self>,
}

impl Block {
    #[must_use]
    pub fn new(kind: BlockKind, span: Span) -> Self {
        Self {
            kind,
            span,
            markers: Vec::new(),
            content: span,
            inlines: Vec::new(),
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_content(mut self, content: Span) -> Self {
        self.content = content;
        self
    }

    #[must_use]
    pub fn with_markers(mut self, markers: Vec<Span>) -> Self {
        self.markers = markers;
        self
    }

    #[must_use]
    pub fn with_children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }

    #[must_use]
    pub fn with_inlines(mut self, inlines: Vec<Inline>) -> Self {
        self.inlines = inlines;
        self
    }

    #[must_use]
    pub const fn heading_level(&self) -> Option<u8> {
        match self.kind {
            BlockKind::Heading { level, .. } => Some(level),
            _ => None,
        }
    }

    #[must_use]
    pub fn language(&self) -> Option<&str> {
        match &self.kind {
            BlockKind::CodeBlock { info, .. } => info.split_whitespace().next(),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_code_block(&self) -> bool {
        matches!(self.kind, BlockKind::CodeBlock { .. })
    }

    #[must_use]
    pub fn table(&self) -> Option<&TableModel> {
        match &self.kind {
            BlockKind::Table(table) => Some(table),
            _ => None,
        }
    }

    #[must_use]
    pub fn frontmatter(&self) -> Option<&Frontmatter> {
        match &self.kind {
            BlockKind::Frontmatter(frontmatter) => Some(frontmatter),
            _ => None,
        }
    }

    #[must_use]
    pub fn task_state(&self) -> Option<bool> {
        match self.kind {
            BlockKind::ListItem { task, .. } => task,
            _ => None,
        }
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
        for inline in &mut self.inlines {
            inline.shift(delta);
        }
        match &mut self.kind {
            BlockKind::Table(table) => shift_table(table, delta),
            BlockKind::Frontmatter(frontmatter) => {
                if let Some(span) = frontmatter.span.shifted(delta) {
                    frontmatter.span = span;
                }
                if let Some(span) = frontmatter.open.shifted(delta) {
                    frontmatter.open = span;
                }
                if let Some(span) = frontmatter.close.shifted(delta) {
                    frontmatter.close = span;
                }
                for property in &mut frontmatter.properties {
                    if let Some(span) = property.span.shifted(delta) {
                        property.span = span;
                    }
                }
            }
            _ => {}
        }
        for child in &mut self.children {
            child.shift(delta);
        }
    }
}

fn shift_table(table: &mut TableModel, delta: isize) {
    if let Some(span) = table.span.shifted(delta) {
        table.span = span;
    }
    if let Some(span) = table.delimiter.shifted(delta) {
        table.delimiter = span;
    }
    for row in &mut table.rows {
        if let Some(span) = row.span.shifted(delta) {
            row.span = span;
        }
        for cell in &mut row.cells {
            if let Some(span) = cell.span.shifted(delta) {
                cell.span = span;
            }
            if let Some(span) = cell.slot.shifted(delta) {
                cell.slot = span;
            }
            for inline in &mut cell.inlines {
                inline.shift(delta);
            }
        }
    }
}
