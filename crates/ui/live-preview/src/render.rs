use margin_markdown::{Block, Document, Inline, Span, TableCell};

use crate::block_live::{self, LineCx};
use crate::block_path::path_of;
use crate::inline_live::{self, Footnotes, Live};
use crate::line_builder::LineBuilder;
use crate::line_view::{LineDecor, LineView, Mode};
use crate::row::Reveal;
use crate::{block_raw, inline_raw};

#[must_use]
pub fn render_line(document: &Document, line: usize, mode: Mode, reveal: &Reveal) -> LineView {
    let span = document.line_span(line);
    let mut builder = LineBuilder::new(document.text(), span);
    let path = path_of(document.blocks(), span);
    let leaf = path.last().copied();
    match mode {
        Mode::Raw => {
            block_raw::decorate(&path, &mut builder);
            for inlines in leaf_inlines(leaf, span) {
                inline_raw::visit(&mut builder, inlines);
            }
            builder.finish(LineDecor::default())
        }
        Mode::LivePreview => {
            let mut cx = LineCx {
                document,
                line,
                touched: reveal.touches(span),
                live: Live {
                    reveal,
                    footnotes: Footnotes::new(document),
                },
            };
            let decor = block_live::decorate(&mut cx, &path, &mut builder);
            for inlines in leaf_inlines(leaf, span) {
                inline_live::visit(&mut cx.live, &mut builder, inlines);
            }
            builder.finish(decor)
        }
    }
}

#[must_use]
pub fn render_cell(document: &Document, cell: &TableCell, reveal: &Reveal) -> LineView {
    let mut builder = LineBuilder::new(document.text(), cell.span);
    let mut live = Live {
        reveal,
        footnotes: Footnotes::new(document),
    };
    inline_live::visit(&mut live, &mut builder, &cell.inlines);
    builder.finish(LineDecor::default())
}

pub(crate) fn leaf_inlines(leaf: Option<&Block>, line: Span) -> Vec<&[Inline]> {
    let Some(leaf) = leaf else {
        return Vec::new();
    };
    let mut out = vec![leaf.inlines.as_slice()];
    if let Some(table) = leaf.table() {
        for row in &table.rows {
            if row.span.start <= line.end && row.span.end >= line.start {
                out.extend(row.cells.iter().map(|cell| cell.inlines.as_slice()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use margin_markdown::{CalloutKind, Fold};

    use super::*;
    use crate::line_view::{
        AtomKind, CodeLinePosition, ContainerKind, InlineStyle, LinkKind, ListMarker,
    };

    fn live(text: &str, line: usize, points: &[usize]) -> LineView {
        let document = Document::parse(text);
        render_line(
            &document,
            line,
            Mode::LivePreview,
            &Reveal::new(points.to_vec()),
        )
    }

    fn styled(view: &LineView, predicate: impl Fn(&InlineStyle) -> bool) -> Vec<&str> {
        view.runs
            .iter()
            .filter(|run| predicate(&run.style))
            .map(|run| view.text.get(run.range.clone()).unwrap_or_default())
            .collect()
    }

    #[test]
    fn inline_markup_hides_off_the_caret_and_shows_on_it() {
        let view = live("a **b** c", 0, &[]);
        assert_eq!(view.text, "a b c");
        assert_eq!(styled(&view, |style| style.bold), vec!["b"]);
        let view = live("a **b** c", 0, &[3]);
        assert_eq!(view.text, "a **b** c");
        assert_eq!(styled(&view, |style| style.markup), vec!["**", "**"]);
        let view = live("a **b** c\nnext", 1, &[3]);
        assert_eq!(view.text, "next");
    }

    #[test]
    fn block_markup_reveals_for_the_whole_line() {
        let view = live("# Title", 0, &[]);
        assert_eq!(view.text, "Title");
        assert_eq!(view.decor.heading, Some(1));
        assert_eq!(view.map.to_source(0), 2);
        let view = live("# Title", 0, &[7]);
        assert_eq!(view.text, "# Title");
        assert_eq!(styled(&view, |style| style.markup), vec!["# "]);
        let view = live("> quote", 0, &[]);
        assert_eq!(view.text, "quote");
        assert_eq!(
            view.decor
                .containers
                .first()
                .map(|container| container.kind),
            Some(ContainerKind::Quote)
        );
    }

    #[test]
    fn raw_mode_shows_every_character() {
        let text = "---\ntitle: x\n---\n# **H** [[a|b]]\n> [!note] t\n- [x] ![i](p.png) $m$\n```js\nx\n```";
        let document = Document::parse(text);
        for line in 0..document.line_count() {
            let view = render_line(&document, line, Mode::Raw, &Reveal::new(vec![]));
            assert_eq!(view.text, document.line_text(line));
            assert!(view.atoms.is_empty() && view.links.is_empty());
            let start = document.line_span(line).start;
            for at in 0..view.text.len() {
                assert_eq!(view.map.to_source(at), start + at);
            }
        }
        let view = render_line(&document, 3, Mode::Raw, &Reveal::new(vec![]));
        assert_eq!(
            styled(&view, |style| style.markup),
            vec!["# **", "**", "[[", "|", "]]"]
        );
    }

    #[test]
    fn links_report_their_display_range() {
        let view = live("see [a](https://x.y) now", 0, &[]);
        assert_eq!(view.text, "see a\u{2009}\u{2197} now");
        let link = view.links.first().expect("a link");
        assert_eq!(link.display, 4..5 + "\u{2009}\u{2197}".len());
        assert_eq!(link.kind, LinkKind::Url("https://x.y".to_owned()));
        assert_eq!(
            styled(&view, |style| style.ornament),
            vec!["\u{2009}\u{2197}"]
        );
        assert!(view.atoms.is_empty());
        let view = live("[[Note|Alias]] and [[Note#Part]]", 0, &[]);
        assert_eq!(view.text, "Alias and Note > Part");
        assert_eq!(view.links.len(), 2);
        assert_eq!(
            styled(&view, |style| style.link),
            vec!["Alias", "Note > Part"]
        );
        assert_eq!(view.map.to_source(17), "[[Note|Alias]] and [[Note#".len());
        let view = live("#tag", 0, &[]);
        assert_eq!(
            view.links.first().map(|link| link.kind.clone()),
            Some(LinkKind::Tag("tag".to_owned()))
        );
    }

    #[test]
    fn lists_hide_markers_into_decor() {
        let view = live("- item", 0, &[]);
        assert_eq!(view.text, "item");
        let list = view.decor.list.expect("list decor");
        assert_eq!((list.depth, list.marker), (0, Some(ListMarker::Bullet)));
        let view = live("1. one\n   - two", 1, &[]);
        assert_eq!(view.text, "two");
        assert_eq!(view.decor.list.map(|list| list.depth), Some(1));
        let view = live("1. one", 0, &[]);
        assert_eq!(
            view.decor.list.and_then(|list| list.marker),
            Some(ListMarker::Ordered("1.".to_owned()))
        );
        let view = live("- [x] done", 0, &[]);
        assert_eq!(view.text, "done");
        assert_eq!(
            view.decor
                .list
                .as_ref()
                .and_then(|list| list.marker.clone()),
            Some(ListMarker::Task(true))
        );
        assert_eq!(
            view.atoms.first().map(|atom| atom.kind.clone()),
            Some(AtomKind::Checkbox { checked: true })
        );
        assert_eq!(styled(&view, |style| style.done), vec!["done"]);
        let view = live("- item", 0, &[0]);
        assert_eq!(view.text, "- item");
        assert_eq!(view.decor.list.and_then(|list| list.marker), None);
    }

    #[test]
    fn callout_headers_collapse_into_decor() {
        let text = "> [!tip]- Title\n> body";
        let view = live(text, 0, &[]);
        assert_eq!(view.text, "");
        let header = view.decor.callout_header.expect("a header");
        assert_eq!(
            (header.kind, header.fold, header.title.as_str()),
            (CalloutKind::Tip, Fold::Collapsed, "Title")
        );
        let body = live(text, 1, &[]);
        assert_eq!(body.text, "body");
        let container = body.decor.containers.first().expect("a container");
        assert_eq!(
            (container.kind, container.first, container.last),
            (ContainerKind::Callout(CalloutKind::Tip), false, true)
        );
        let view = live(text, 0, &[text.len()]);
        assert_eq!(view.text, "[!tip]- Title");
        assert!(view.decor.callout_header.is_none());
        let view = live(":::warning Careful\ntext\n:::", 0, &[]);
        assert_eq!(
            view.decor.callout_header.map(|header| header.kind),
            Some(CalloutKind::Warning)
        );
    }

    #[test]
    fn code_fences_hide_off_the_block() {
        let text = "```rust\nfn x\n```";
        let view = live(text, 0, &[]);
        assert_eq!(view.text, "");
        let code = view.decor.code.expect("code line");
        assert_eq!(
            (code.position, code.fence_hidden, code.language.as_deref()),
            (CodeLinePosition::OpenFence, true, Some("rust"))
        );
        let view = live(text, 0, &[9]);
        assert_eq!(view.text, "```rust");
        assert_eq!(styled(&view, |style| style.markup), vec!["```"]);
        assert_eq!(
            live(text, 2, &[]).decor.code.map(|code| code.position),
            Some(CodeLinePosition::CloseFence)
        );
    }

    #[test]
    fn atoms_replace_math_html_images_and_footnotes() {
        let view = live("a $x^2$ <u>u</u> b<br>c", 0, &[]);
        assert_eq!(view.text, "a  u bc");
        assert_eq!(
            view.atoms.first().map(|atom| atom.kind.clone()),
            Some(AtomKind::Math {
                tex: "x^2".to_owned()
            })
        );
        assert_eq!(
            view.atoms
                .get(1)
                .map(|atom| (atom.kind.clone(), atom.display)),
            Some((AtomKind::LineBreak, 6))
        );
        assert_eq!(styled(&view, |style| style.underline), vec!["u"]);
        let view = live("![alt|200x100](a.png) ![[pic.png|300]]", 0, &[]);
        let image = view.atoms.first().expect("an image");
        assert_eq!(
            image.kind,
            AtomKind::Image {
                destination: "a.png".to_owned(),
                alt: "alt".to_owned()
            }
        );
        assert_eq!(
            image.size.map(|size| (size.width, size.height)),
            Some((200, Some(100)))
        );
        assert_eq!(
            view.atoms
                .get(1)
                .and_then(|atom| atom.size)
                .map(|size| size.width),
            Some(300)
        );
        let text = "a[^n]\n\n[^n]: note";
        assert_eq!(
            live(text, 0, &[])
                .atoms
                .first()
                .map(|atom| atom.kind.clone()),
            Some(AtomKind::FootnoteReference {
                id: "n".to_owned(),
                number: 1
            })
        );
        let definition = live(text, 2, &[]);
        assert_eq!(definition.decor.footnote_definition.as_deref(), Some("n"));
        assert_eq!(
            definition.atoms.first().map(|atom| atom.kind.clone()),
            Some(AtomKind::FootnoteDefinition {
                id: "n".to_owned(),
                number: Some(1)
            })
        );
    }

    #[test]
    fn cells_use_the_inline_rules() {
        let document = Document::parse("| **a** | b |\n| --- | --- |\n| c | d |");
        let table = document.tables().into_iter().next().expect("a table");
        let cell = table
            .rows
            .first()
            .and_then(|row| row.cells.first())
            .expect("a cell");
        let view = render_cell(&document, cell, &Reveal::default());
        assert_eq!(view.text, "a");
        assert_eq!(styled(&view, |style| style.bold), vec!["a"]);
        let view = render_cell(&document, cell, &Reveal::new(vec![cell.span.start]));
        assert_eq!(view.text, "**a**");
    }
}
