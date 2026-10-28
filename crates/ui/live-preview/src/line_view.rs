use std::ops::Range;

use margin_markdown::{CalloutKind, EmbedTarget, Fold, Span, WikiLink};

use crate::offset_map::OffsetMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    LivePreview,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntaxRole {
    Keyword,
    String,
    Number,
    Function,
    Comment,
    Attribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HighlightColor {
    Yellow,
    Blue,
    Green,
    Purple,
    Red,
    Orange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Script {
    Normal,
    Superscript,
    Subscript,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent text attribute the renderer combines"
)]
pub struct InlineStyle {
    pub heading: Option<u8>,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
    pub code: bool,
    pub keyboard: bool,
    pub highlight: Option<HighlightColor>,
    pub link: bool,
    pub tag: bool,
    pub markup: bool,
    pub syntax: Option<SyntaxRole>,
    pub script: Script,
    pub small: bool,
    pub quote: bool,
    pub done: bool,
    pub ornament: bool,
}

impl Default for InlineStyle {
    fn default() -> Self {
        Self {
            heading: None,
            bold: false,
            italic: false,
            strike: false,
            underline: false,
            code: false,
            keyboard: false,
            highlight: None,
            link: false,
            tag: false,
            markup: false,
            syntax: None,
            script: Script::Normal,
            small: false,
            quote: false,
            done: false,
            ornament: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyledRun {
    pub range: Range<usize>,
    pub style: InlineStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Wiki(WikiLink),
    Url(String),
    Tag(String),
    Footnote(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkHit {
    pub display: Range<usize>,
    pub source: Span,
    pub kind: LinkKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomKind {
    Embed(EmbedTarget),
    Image { destination: String, alt: String },
    Math { tex: String },
    Checkbox { checked: bool },
    FootnoteReference { id: String, number: usize },
    FootnoteDefinition { id: String, number: Option<usize> },
    LineBreak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    pub display: usize,
    pub source: Span,
    pub kind: AtomKind,
    pub size: Option<ImageSize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListMarker {
    Bullet,
    Ordered(String),
    Task(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListDecor {
    pub depth: usize,
    pub marker: Option<ListMarker>,
    pub marker_source: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    Quote,
    Callout(CalloutKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Container {
    pub kind: ContainerKind,
    pub first: bool,
    pub last: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalloutHeader {
    pub kind: CalloutKind,
    pub fold: Fold,
    pub block: Span,
    pub title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeLinePosition {
    OpenFence,
    Body,
    CloseFence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeLine {
    pub language: Option<String>,
    pub position: CodeLinePosition,
    pub block: Span,
    pub content: Span,
    pub fence_hidden: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineDecor {
    pub heading: Option<u8>,
    pub containers: Vec<Container>,
    pub callout_header: Option<CalloutHeader>,
    pub list: Option<ListDecor>,
    pub code: Option<CodeLine>,
    pub rule: bool,
    pub comment: bool,
    pub footnote_definition: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineView {
    pub text: String,
    pub map: OffsetMap,
    pub runs: Vec<StyledRun>,
    pub links: Vec<LinkHit>,
    pub atoms: Vec<Atom>,
    pub decor: LineDecor,
}
