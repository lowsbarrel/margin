use margin_markdown::{Document, EmbedTarget, FootnoteIndex, Inline, InlineKind, Span, WikiLink};

use crate::html_tags::{self, HtmlRole};
use crate::line_builder::{LineBuilder, Mark};
use crate::line_view::{AtomKind, ImageSize, LinkKind};
use crate::row::Reveal;

#[derive(Debug)]
pub(crate) struct Footnotes<'a> {
    document: &'a Document,
    index: Option<FootnoteIndex>,
}

impl<'a> Footnotes<'a> {
    pub(crate) const fn new(document: &'a Document) -> Self {
        Self {
            document,
            index: None,
        }
    }

    pub(crate) fn number(&mut self, id: &str) -> Option<usize> {
        let document = self.document;
        self.index
            .get_or_insert_with(|| document.footnotes())
            .number(id)
    }
}

#[derive(Debug)]
pub(crate) struct Live<'a> {
    pub reveal: &'a Reveal,
    pub footnotes: Footnotes<'a>,
}

pub(crate) fn visit(live: &mut Live<'_>, builder: &mut LineBuilder<'_>, inlines: &[Inline]) {
    let roles = html_tags::roles(builder.source(), inlines);
    for (inline, role) in inlines.iter().zip(roles) {
        if builder.intersects(inline.span) {
            one(live, builder, inline, role);
        }
    }
}

fn reveal_or_hide(builder: &mut LineBuilder<'_>, near: bool, span: Span) {
    if near {
        builder.mark(span, Mark::Markup);
    } else {
        builder.hide(span);
    }
}

fn markers(builder: &mut LineBuilder<'_>, near: bool, inline: &Inline) {
    for marker in &inline.markers {
        reveal_or_hide(builder, near, *marker);
    }
}

fn one(live: &mut Live<'_>, builder: &mut LineBuilder<'_>, inline: &Inline, role: HtmlRole) {
    let near = live.reveal.touches(inline.span);
    let span = inline.span;
    match &inline.kind {
        InlineKind::Emphasis
        | InlineKind::Strong
        | InlineKind::Strikethrough
        | InlineKind::Highlight => {
            let mark = match inline.kind {
                InlineKind::Emphasis => Mark::Italic,
                InlineKind::Strong => Mark::Bold,
                InlineKind::Strikethrough => Mark::Strike,
                _ => Mark::Highlight,
            };
            builder.mark(span, mark);
            markers(builder, near, inline);
            visit(live, builder, &inline.children);
        }
        InlineKind::Code => {
            builder.mark(span, Mark::Code);
            markers(builder, near, inline);
        }
        InlineKind::Link(target) => link(live, builder, inline, &target.destination, near),
        InlineKind::Autolink { destination, .. } => {
            builder.mark(span, Mark::Link);
            builder.link(span, LinkKind::Url(destination.clone()));
            markers(builder, near, inline);
        }
        InlineKind::Image(target) => {
            if near {
                builder.mark(span, Mark::Code);
                return;
            }
            let label = inline.content.slice(builder.source()).replace("\\|", "|");
            let (alt, size) = match label.rsplit_once('|') {
                Some((alt, spec)) => match parse_size(spec) {
                    Some(size) => (alt.to_owned(), Some(size)),
                    None => (label.clone(), None),
                },
                None => (label.clone(), None),
            };
            let kind = AtomKind::Image {
                destination: target.destination.clone(),
                alt,
            };
            builder.atom(span, kind, size);
        }
        InlineKind::WikiLink(target) => wiki(builder, inline, target, near),
        InlineKind::Embed(target) => embed(builder, span, target, near),
        InlineKind::Math => {
            if near {
                builder.mark(span, Mark::Markup);
            } else {
                let tex = inline.content.slice(builder.source()).to_owned();
                builder.atom(span, AtomKind::Math { tex }, None);
            }
        }
        InlineKind::Comment => builder.mark(span, Mark::Markup),
        InlineKind::FootnoteReference { id } => {
            if near {
                return;
            }
            if let Some(number) = live.footnotes.number(id) {
                builder.link(span, LinkKind::Footnote(id.clone()));
                let kind = AtomKind::FootnoteReference {
                    id: id.clone(),
                    number,
                };
                builder.atom(span, kind, None);
            }
        }
        InlineKind::Tag { name } => {
            builder.mark(span, Mark::Tag);
            builder.link(span, LinkKind::Tag(name.clone()));
        }
        InlineKind::Html { .. } => html(live, builder, span, role),
        InlineKind::Escape if !near => markers(builder, near, inline),
        InlineKind::Escape | InlineKind::Text | InlineKind::SoftBreak | InlineKind::HardBreak => {}
    }
}

fn html(live: &Live<'_>, builder: &mut LineBuilder<'_>, span: Span, role: HtmlRole) {
    match role {
        HtmlRole::Break => {
            if live.reveal.touches(span) {
                builder.mark(span, Mark::Code);
            } else {
                builder.atom(span, AtomKind::LineBreak, None);
            }
        }
        HtmlRole::Open { close, mark } => {
            if live.reveal.touches(Span::new(span.start, close.end)) {
                builder.mark(span, Mark::Code);
            } else {
                builder.hide(span);
                builder.mark(Span::new(span.end, close.start), mark);
            }
        }
        HtmlRole::Close { open } => {
            if live.reveal.touches(Span::new(open.start, span.end)) {
                builder.mark(span, Mark::Code);
            } else {
                builder.hide(span);
            }
        }
        HtmlRole::Other => builder.mark(span, Mark::Code),
    }
}

pub(crate) fn is_inline_link(inline: &Inline) -> bool {
    inline.markers.len() == 4
}

pub(crate) fn link_tail(inline: &Inline) -> Option<Span> {
    let separator = inline.markers.get(1)?;
    Some(Span::new(separator.start, inline.span.end))
}

fn link(
    live: &mut Live<'_>,
    builder: &mut LineBuilder<'_>,
    inline: &Inline,
    destination: &str,
    near: bool,
) {
    if !is_inline_link(inline) {
        visit(live, builder, &inline.children);
        return;
    }
    let span = inline.span;
    builder.mark(span, Mark::Link);
    builder.link(span, LinkKind::Url(destination.to_owned()));
    if let Some(open) = inline.markers.first() {
        reveal_or_hide(builder, near, *open);
    }
    if let Some(tail) = link_tail(inline) {
        reveal_or_hide(builder, near, tail);
    }
    if !near
        && ["http:", "https:", "mailto:"]
            .iter()
            .any(|scheme| destination.starts_with(scheme))
    {
        builder.insert(span.end, "\u{2009}\u{2197}", Mark::Ornament);
    }
    visit(live, builder, &inline.children);
}

pub(crate) struct WikiParts {
    pub open: Span,
    pub target: Span,
    pub pipe: Option<Span>,
    pub close: Span,
}

pub(crate) fn wiki_parts(source: &str, inline: &Inline) -> Option<WikiParts> {
    let open = *inline.markers.first()?;
    let close = *inline.markers.get(1)?;
    let inner = Span::new(open.end, close.start);
    let pipe = inner
        .slice(source)
        .find('|')
        .map(|offset| Span::new(inner.start + offset, inner.start + offset + 1));
    let target = Span::new(inner.start, pipe.map_or(inner.end, |pipe| pipe.start));
    Some(WikiParts {
        open,
        target,
        pipe,
        close,
    })
}

fn wiki(builder: &mut LineBuilder<'_>, inline: &Inline, target: &WikiLink, near: bool) {
    let span = inline.span;
    builder.mark(span, Mark::Link);
    builder.link(span, LinkKind::Wiki(target.clone()));
    let Some(parts) = wiki_parts(builder.source(), inline) else {
        return;
    };
    reveal_or_hide(builder, near, parts.open);
    reveal_or_hide(builder, near, parts.close);
    if let Some(pipe) = parts.pipe {
        reveal_or_hide(builder, near, Span::new(parts.target.start, pipe.end));
        return;
    }
    if near {
        return;
    }
    let text = parts.target.slice(builder.source());
    for (offset, _) in text.match_indices('#') {
        let at = Span::new(parts.target.start + offset, parts.target.start + offset + 1);
        builder.hide(at);
        if offset > 0 {
            builder.insert(at.start, " > ", Mark::Link);
        }
    }
}

fn embed(builder: &mut LineBuilder<'_>, span: Span, target: &EmbedTarget, near: bool) {
    if near {
        builder.mark(span, Mark::Code);
        return;
    }
    let text = span.slice(builder.source());
    let inner = text
        .get(3..text.len().saturating_sub(2))
        .unwrap_or_default()
        .replace("\\|", "|");
    let size = inner.split_once('|').and_then(|(_, spec)| parse_size(spec));
    builder.atom(span, AtomKind::Embed(target.clone()), size);
}

pub(crate) fn parse_size(spec: &str) -> Option<ImageSize> {
    let spec = spec.trim();
    let (width, height) = match spec.split_once('x') {
        Some((width, height)) => (width, Some(height)),
        None => (spec, None),
    };
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(width) || height.is_some_and(|height| !digits(height)) {
        return None;
    }
    Some(ImageSize {
        width: width.parse().ok()?,
        height: match height {
            Some(height) => Some(height.parse().ok()?),
            None => None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_accept_width_or_width_by_height() {
        assert_eq!(
            parse_size(" 300 "),
            Some(ImageSize {
                width: 300,
                height: None
            })
        );
        assert_eq!(
            parse_size("300x200"),
            Some(ImageSize {
                width: 300,
                height: Some(200)
            })
        );
        assert_eq!(parse_size("wide"), None);
        assert_eq!(parse_size("300x"), None);
    }
}
