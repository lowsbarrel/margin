use std::iter::Peekable;
use std::vec::IntoIter;

use margin_markdown::Span;

use crate::line_view::{
    Atom, AtomKind, HighlightColor, ImageSize, InlineStyle, LineDecor, LineView, LinkHit, LinkKind,
    Script, StyledRun, SyntaxRole,
};
use crate::offset_map::OffsetMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Heading(u8),
    Bold,
    Italic,
    Strike,
    Underline,
    Code,
    Keyboard,
    Highlight,
    Link,
    Tag,
    Markup,
    Syntax(SyntaxRole),
    Script(Script),
    Small,
    Quote,
    Done,
    Ornament,
}

impl Mark {
    fn apply(self, style: &mut InlineStyle) {
        match self {
            Self::Heading(level) => style.heading = Some(level),
            Self::Bold => style.bold = true,
            Self::Italic => style.italic = true,
            Self::Strike => style.strike = true,
            Self::Underline => style.underline = true,
            Self::Code => style.code = true,
            Self::Keyboard => style.keyboard = true,
            Self::Highlight => style.highlight = Some(HighlightColor::Yellow),
            Self::Link => style.link = true,
            Self::Tag => style.tag = true,
            Self::Markup => style.markup = true,
            Self::Syntax(role) => style.syntax = Some(role),
            Self::Script(script) => style.script = script,
            Self::Small => style.small = true,
            Self::Quote => style.quote = true,
            Self::Done => style.done = true,
            Self::Ornament => style.ornament = true,
        }
    }
}

#[derive(Debug)]
struct PendingAtom {
    source: Span,
    kind: AtomKind,
    size: Option<ImageSize>,
}

#[derive(Debug)]
struct Insert {
    at: usize,
    text: &'static str,
    mark: Mark,
}

#[derive(Debug)]
pub(crate) struct LineBuilder<'a> {
    source: &'a str,
    line: Span,
    hidden: Vec<Span>,
    atoms: Vec<PendingAtom>,
    marks: Vec<(Span, Mark)>,
    links: Vec<(Span, LinkKind)>,
    inserts: Vec<Insert>,
}

impl<'a> LineBuilder<'a> {
    pub(crate) const fn new(source: &'a str, line: Span) -> Self {
        Self {
            source,
            line,
            hidden: Vec::new(),
            atoms: Vec::new(),
            marks: Vec::new(),
            links: Vec::new(),
            inserts: Vec::new(),
        }
    }

    pub(crate) const fn source(&self) -> &'a str {
        self.source
    }

    pub(crate) const fn line(&self) -> Span {
        self.line
    }

    pub(crate) const fn intersects(&self, span: Span) -> bool {
        span.start <= self.line.end && span.end >= self.line.start
    }

    fn clip(&self, span: Span) -> Option<Span> {
        let start = span.start.max(self.line.start);
        let end = span.end.min(self.line.end);
        (start < end).then_some(Span::new(start, end))
    }

    pub(crate) fn hide(&mut self, span: Span) {
        if let Some(span) = self.clip(span) {
            self.hidden.push(span);
        }
    }

    pub(crate) fn mark(&mut self, span: Span, mark: Mark) {
        if let Some(span) = self.clip(span) {
            self.marks.push((span, mark));
        }
    }

    pub(crate) fn mark_line(&mut self, mark: Mark) {
        self.mark(self.line, mark);
    }

    pub(crate) fn atom(&mut self, source: Span, kind: AtomKind, size: Option<ImageSize>) {
        if source.start < self.line.start || source.start > self.line.end {
            return;
        }
        let source = Span::new(source.start, source.end.min(self.line.end));
        self.atoms.push(PendingAtom { source, kind, size });
    }

    pub(crate) fn insert(&mut self, at: usize, text: &'static str, mark: Mark) {
        if (self.line.start..=self.line.end).contains(&at) {
            self.inserts.push(Insert { at, text, mark });
        }
    }

    pub(crate) fn link(&mut self, span: Span, kind: LinkKind) {
        if self.intersects(span) {
            self.links.push((span, kind));
        }
    }

    pub(crate) fn finish(self, decor: LineDecor) -> LineView {
        let mut hidden = self.hidden;
        hidden.extend(
            self.atoms
                .iter()
                .filter(|atom| !atom.source.is_empty())
                .map(|atom| atom.source),
        );
        hidden.sort_unstable();
        let mut inserts = self.inserts;
        inserts.sort_by_key(|insert| insert.at);
        let mut text = Output {
            source: self.source,
            text: String::with_capacity(self.line.len()),
            map: OffsetMap::new(self.line.start, self.line.end),
            inserted: Vec::new(),
            pending: inserts.into_iter().peekable(),
            cursor: self.line.start,
        };
        for span in &hidden {
            text.visible_to(span.start);
            text.cursor = text.cursor.max(span.end);
        }
        text.visible_to(self.line.end);
        let Output {
            text,
            map,
            inserted,
            ..
        } = text;
        let runs = runs(&self.marks, &inserted, &map, text.len());
        let mut atoms: Vec<Atom> = self
            .atoms
            .into_iter()
            .map(|atom| Atom {
                display: map.to_display(atom.source.start),
                source: atom.source,
                kind: atom.kind,
                size: atom.size,
            })
            .collect();
        atoms.sort_by_key(|atom| atom.source.start);
        let links = self
            .links
            .into_iter()
            .map(|(source, kind)| LinkHit {
                display: map.to_display(source.start.max(self.line.start))
                    ..map.to_display(source.end.min(self.line.end)),
                source,
                kind,
            })
            .collect();
        LineView {
            text,
            map,
            runs,
            links,
            atoms,
            decor,
        }
    }
}

struct Output<'a> {
    source: &'a str,
    text: String,
    map: OffsetMap,
    inserted: Vec<(usize, usize, Mark)>,
    pending: Peekable<IntoIter<Insert>>,
    cursor: usize,
}

impl Output<'_> {
    fn visible_to(&mut self, end: usize) {
        let end = end.max(self.cursor);
        while let Some(insert) = self.pending.next_if(|insert| insert.at <= end) {
            let at = insert.at.max(self.cursor);
            self.visible(at);
            let start = self.text.len();
            self.text.push_str(insert.text);
            self.inserted.push((start, self.text.len(), insert.mark));
        }
        self.visible(end);
    }

    fn visible(&mut self, end: usize) {
        if end <= self.cursor {
            return;
        }
        let span = Span::new(self.cursor, end);
        self.map.push(self.text.len(), span.start, span.len());
        self.text.push_str(span.slice(self.source));
        self.cursor = end;
    }
}

fn runs(
    marks: &[(Span, Mark)],
    inserted: &[(usize, usize, Mark)],
    map: &OffsetMap,
    len: usize,
) -> Vec<StyledRun> {
    let ranges: Vec<(usize, usize, Mark)> = marks
        .iter()
        .map(|(span, mark)| (map.to_display(span.start), map.to_display(span.end), *mark))
        .chain(inserted.iter().copied())
        .filter(|(start, end, _)| start < end)
        .collect();
    let mut bounds: Vec<usize> = ranges
        .iter()
        .flat_map(|(start, end, _)| [*start, *end])
        .collect();
    bounds.push(0);
    bounds.push(len);
    bounds.sort_unstable();
    bounds.dedup();
    let mut out: Vec<StyledRun> = Vec::new();
    for pair in bounds.windows(2) {
        let [start, end] = *pair else {
            continue;
        };
        let mut style = InlineStyle::default();
        for (from, to, mark) in &ranges {
            if *from <= start && *to >= end {
                mark.apply(&mut style);
            }
        }
        if style == InlineStyle::default() {
            continue;
        }
        if let Some(last) = out.last_mut()
            && last.range.end == start
            && last.style == style
        {
            last.range.end = end;
            continue;
        }
        out.push(StyledRun {
            range: start..end,
            style,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_markup_leaves_the_text_and_combines_styles() {
        let source = "**[a](u)**";
        let mut builder = LineBuilder::new(source, Span::new(0, source.len()));
        builder.hide(Span::new(0, 3));
        builder.hide(Span::new(4, 10));
        builder.mark(Span::new(0, 10), Mark::Bold);
        builder.mark(Span::new(2, 8), Mark::Link);
        let view = builder.finish(LineDecor::default());
        assert_eq!(view.text, "a");
        assert_eq!(view.runs.len(), 1);
        let style = view.runs.first().expect("one run").style;
        assert!(style.bold && style.link);
    }

    #[test]
    fn atoms_are_zero_width_at_their_display_offset() {
        let source = "a $x$ b";
        let mut builder = LineBuilder::new(source, Span::new(0, source.len()));
        builder.atom(
            Span::new(2, 5),
            AtomKind::Math {
                tex: "x".to_owned(),
            },
            None,
        );
        let view = builder.finish(LineDecor::default());
        assert_eq!(view.text, "a  b");
        assert_eq!(view.atoms.first().map(|atom| atom.display), Some(2));
        assert_eq!(view.map.to_source(2), 2);
        assert_eq!(view.map.to_display(5), 2);
        assert_eq!(view.map.to_source(3), 6);
    }

    #[test]
    fn inserted_text_has_no_source_and_keeps_its_mark() {
        let source = "[a](u) b#c";
        let mut builder = LineBuilder::new(source, Span::new(0, source.len()));
        builder.hide(Span::new(0, 1));
        builder.hide(Span::new(2, 6));
        builder.insert(6, "\u{2197}", Mark::Ornament);
        builder.hide(Span::new(8, 9));
        builder.insert(8, " > ", Mark::Link);
        let view = builder.finish(LineDecor::default());
        assert_eq!(view.text, "a\u{2197} b > c");
        let ornament = view.runs.first().expect("an ornament run");
        assert!(ornament.style.ornament);
        assert_eq!(view.map.to_source(0), 1);
        assert_eq!(view.map.to_source(view.text.len()), source.len());
        assert_eq!(view.map.to_display(9), view.text.len() - 1);
    }
}
