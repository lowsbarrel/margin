use margin_markdown::{Document, Inline, InlineKind, Span};

fn inlines_of(text: &str) -> Vec<Inline> {
    let document = Document::parse(text);
    document
        .blocks()
        .first()
        .expect("the note has a block")
        .inlines
        .clone()
}

fn first_inline(text: &str) -> Inline {
    inlines_of(text).into_iter().next().expect("an inline")
}

fn kinds(inlines: &[Inline]) -> Vec<&InlineKind> {
    inlines.iter().map(|inline| &inline.kind).collect()
}

#[test]
fn emphasis_and_strong_ranges() {
    let emphasis = first_inline("*a*");
    assert_eq!(emphasis.kind, InlineKind::Emphasis);
    assert_eq!(emphasis.span, Span::new(0, 3));
    assert_eq!(emphasis.markers, vec![Span::new(0, 1), Span::new(2, 3)]);
    assert_eq!(emphasis.content, Span::new(1, 2));

    let strong = first_inline("**a**");
    assert_eq!(strong.kind, InlineKind::Strong);
    assert_eq!(strong.markers, vec![Span::new(0, 2), Span::new(3, 5)]);
    assert_eq!(strong.content, Span::new(2, 3));
}

#[test]
fn triple_emphasis_nests_strong_inside_emphasis() {
    let outer = first_inline("***a***");
    assert_eq!(outer.kind, InlineKind::Emphasis);
    let inner = outer.children.first().expect("a nested strong");
    assert_eq!(inner.kind, InlineKind::Strong);
    assert_eq!(inner.content.slice("***a***"), "a");
}

#[test]
fn strong_wraps_emphasis() {
    let outer = first_inline("**a *b* c**");
    assert_eq!(outer.kind, InlineKind::Strong);
    assert_eq!(outer.children.len(), 3);
    assert_eq!(
        outer.children.get(1).expect("an emphasis").kind,
        InlineKind::Emphasis
    );
}

#[test]
fn intraword_underscore_is_literal() {
    let inlines = inlines_of("snake_case_name");
    assert_eq!(inlines.len(), 1);
    assert_eq!(inlines.first().expect("text").kind, InlineKind::Text);
}

#[test]
fn strikethrough_and_highlight() {
    let strike = first_inline("~~a~~");
    assert_eq!(strike.kind, InlineKind::Strikethrough);
    assert_eq!(strike.markers, vec![Span::new(0, 2), Span::new(3, 5)]);

    let highlight = first_inline("==a==");
    assert_eq!(highlight.kind, InlineKind::Highlight);
    assert_eq!(highlight.markers, vec![Span::new(0, 2), Span::new(3, 5)]);

    assert_eq!(first_inline("~a~").kind, InlineKind::Text);
    assert_eq!(first_inline("===a===").kind, InlineKind::Text);
}

#[test]
fn code_span_ranges() {
    let code = first_inline("`a`");
    assert_eq!(code.kind, InlineKind::Code);
    assert_eq!(code.markers, vec![Span::new(0, 1), Span::new(2, 3)]);
    assert_eq!(code.content, Span::new(1, 2));

    let double = first_inline("``a``");
    assert_eq!(double.kind, InlineKind::Code);
    assert_eq!(double.markers, vec![Span::new(0, 2), Span::new(3, 5)]);

    let padded = first_inline("` a `");
    assert_eq!(padded.content, Span::new(2, 3));
}

#[test]
fn inline_link_ranges() {
    let link = first_inline("[a](url)");
    match &link.kind {
        InlineKind::Link(target) => assert_eq!(target.destination, "url"),
        other => panic!("expected a link, got {other:?}"),
    }
    assert_eq!(link.span, Span::new(0, 8));
    assert_eq!(link.content, Span::new(1, 2));
    assert_eq!(
        link.markers,
        vec![
            Span::new(0, 1),
            Span::new(2, 4),
            Span::new(4, 7),
            Span::new(7, 8)
        ]
    );
    assert_eq!(link.children.len(), 1);
}

#[test]
fn link_with_title() {
    let link = first_inline("[a](url \"t\")");
    match &link.kind {
        InlineKind::Link(target) => {
            assert_eq!(target.destination, "url");
            assert_eq!(target.title.as_deref(), Some("t"));
        }
        other => panic!("expected a link, got {other:?}"),
    }
}

#[test]
fn reference_links_resolve() {
    let full = inlines_of("[a][b]\n\n[b]: url \"t\"\n");
    match &full.first().expect("a link").kind {
        InlineKind::Link(target) => {
            assert_eq!(target.destination, "url");
            assert_eq!(target.title.as_deref(), Some("t"));
        }
        other => panic!("expected a link, got {other:?}"),
    }
    assert_eq!(
        full.first().expect("a link").markers,
        vec![Span::new(0, 1), Span::new(2, 6)]
    );

    let collapsed = inlines_of("[b][]\n\n[b]: url\n");
    assert!(matches!(
        collapsed.first().expect("a link").kind,
        InlineKind::Link(_)
    ));

    let shortcut = inlines_of("[b]\n\n[b]: url\n");
    assert!(matches!(
        shortcut.first().expect("a link").kind,
        InlineKind::Link(_)
    ));

    let unknown = inlines_of("[b]\n");
    assert_eq!(unknown.first().expect("text").kind, InlineKind::Text);
}

#[test]
fn autolinks_and_bare_links() {
    let angle = first_inline("<https://x>");
    match &angle.kind {
        InlineKind::Autolink { destination, email } => {
            assert_eq!(destination, "https://x");
            assert!(!email);
        }
        other => panic!("expected an autolink, got {other:?}"),
    }
    assert_eq!(angle.markers, vec![Span::new(0, 1), Span::new(10, 11)]);

    let inlines = inlines_of("see https://x now");
    match &inlines.get(1).expect("an autolink").kind {
        InlineKind::Autolink { destination, .. } => assert_eq!(destination, "https://x"),
        other => panic!("expected an autolink, got {other:?}"),
    }

    let www = inlines_of("www.x.com");
    match &www.first().expect("an autolink").kind {
        InlineKind::Autolink { destination, .. } => assert_eq!(destination, "http://www.x.com"),
        other => panic!("expected an autolink, got {other:?}"),
    }

    let email = inlines_of("me@x.com");
    match &email.first().expect("an autolink").kind {
        InlineKind::Autolink { destination, email } => {
            assert_eq!(destination, "mailto:me@x.com");
            assert!(*email);
        }
        other => panic!("expected an autolink, got {other:?}"),
    }
}

#[test]
fn trailing_punctuation_stays_out_of_a_bare_link() {
    let inlines = inlines_of("see https://x.");
    assert_eq!(inlines.len(), 3);
    assert_eq!(
        inlines.get(2).expect("text").span.slice("see https://x."),
        "."
    );
}

#[test]
fn image_ranges() {
    let image = first_inline("![alt](src.png)");
    match &image.kind {
        InlineKind::Image(target) => assert_eq!(target.destination, "src.png"),
        other => panic!("expected an image, got {other:?}"),
    }
    assert_eq!(image.span, Span::new(0, 15));
    assert_eq!(image.content, Span::new(2, 5));
    assert_eq!(
        image.markers,
        vec![
            Span::new(0, 2),
            Span::new(5, 7),
            Span::new(7, 14),
            Span::new(14, 15)
        ]
    );
}

#[test]
fn wiki_link_with_alias() {
    let wiki = first_inline("[[Note|Alias]]");
    match &wiki.kind {
        InlineKind::WikiLink(link) => {
            assert_eq!(link.target, "Note");
            assert_eq!(link.alias.as_deref(), Some("Alias"));
            assert_eq!(link.label(), "Alias");
            assert_eq!(link.heading, None);
        }
        other => panic!("expected a wiki link, got {other:?}"),
    }
    assert_eq!(wiki.content, Span::new(7, 12));
    assert_eq!(
        wiki.markers,
        vec![Span::new(0, 2), Span::new(12, 14), Span::new(6, 7)]
    );
}

#[test]
fn wiki_link_with_heading_and_block() {
    let heading = first_inline("[[Note#Head]]");
    match &heading.kind {
        InlineKind::WikiLink(link) => {
            assert_eq!(link.target, "Note");
            assert_eq!(link.heading.as_deref(), Some("Head"));
            assert_eq!(link.block, None);
        }
        other => panic!("expected a wiki link, got {other:?}"),
    }
    assert_eq!(heading.content, Span::new(2, 11));
    assert_eq!(heading.markers.get(2), Some(&Span::new(6, 7)));

    let block = first_inline("[[Note#^b1]]");
    match &block.kind {
        InlineKind::WikiLink(link) => {
            assert_eq!(link.heading, None);
            assert_eq!(link.block.as_deref(), Some("b1"));
        }
        other => panic!("expected a wiki link, got {other:?}"),
    }

    assert_eq!(first_inline("[[]]").kind, InlineKind::Text);
}

#[test]
fn embed_with_size() {
    let embed = first_inline("![[img.png|100]]");
    match &embed.kind {
        InlineKind::Embed(target) => {
            assert_eq!(target.target, "img.png");
            assert_eq!(target.size, Some(100));
        }
        other => panic!("expected an embed, got {other:?}"),
    }
    assert_eq!(embed.markers.first(), Some(&Span::new(0, 3)));
    assert_eq!(embed.markers.get(2), Some(&Span::new(10, 11)));
}

#[test]
fn inline_math_and_comment() {
    let math = first_inline("$x$");
    assert_eq!(math.kind, InlineKind::Math);
    assert_eq!(math.content, Span::new(1, 2));
    assert_eq!(math.markers, vec![Span::new(0, 1), Span::new(2, 3)]);

    assert_eq!(first_inline("a $$x$$").kind, InlineKind::Text);
    assert_eq!(first_inline("$ x$").kind, InlineKind::Text);

    let comment = first_inline("%%hidden%%");
    assert_eq!(comment.kind, InlineKind::Comment);
    assert_eq!(comment.content, Span::new(2, 8));
    assert_eq!(comment.markers, vec![Span::new(0, 2), Span::new(8, 10)]);
}

#[test]
fn footnote_reference_markers() {
    let inlines = inlines_of("see[^a]\n\n[^a]: body\n");
    let reference = inlines.get(1).expect("a footnote reference");
    assert_eq!(reference.markers, vec![Span::new(3, 5), Span::new(6, 7)]);
    assert_eq!(reference.content, Span::new(5, 6));
}

#[test]
fn obsidian_tags() {
    let inlines = inlines_of("a #tag/b more");
    match &inlines.get(1).expect("a tag").kind {
        InlineKind::Tag { name } => assert_eq!(name, "tag/b"),
        other => panic!("expected a tag, got {other:?}"),
    }
    assert_eq!(
        inlines.get(1).expect("a tag").markers,
        vec![Span::new(2, 3)]
    );

    assert_eq!(first_inline("#2024").kind, InlineKind::Text);
    assert!(matches!(first_inline("#tag/").kind, InlineKind::Tag { .. }));
}

#[test]
fn backslash_escapes() {
    let escape = first_inline("\\*");
    assert_eq!(escape.kind, InlineKind::Escape);
    assert_eq!(escape.markers, vec![Span::new(0, 1)]);
    assert_eq!(escape.content, Span::new(1, 2));

    let literal = inlines_of("a\\b");
    assert_eq!(literal.len(), 1);
}

#[test]
fn breaks() {
    let soft = inlines_of("a\nb");
    assert_eq!(
        kinds(&soft),
        vec![&InlineKind::Text, &InlineKind::SoftBreak, &InlineKind::Text]
    );

    let hard = inlines_of("a  \nb");
    assert_eq!(
        kinds(&hard),
        vec![&InlineKind::Text, &InlineKind::HardBreak, &InlineKind::Text]
    );
    assert_eq!(hard.get(1).expect("a break").markers, vec![Span::new(1, 3)]);

    let escaped = inlines_of("a\\\nb");
    assert_eq!(escaped.get(1).expect("a break").kind, InlineKind::HardBreak);
}

#[test]
fn html_tags() {
    match &first_inline("<kbd>").kind {
        InlineKind::Html { name, closing } => {
            assert_eq!(name, "kbd");
            assert!(!closing);
        }
        other => panic!("expected html, got {other:?}"),
    }
    match &first_inline("</KBD>").kind {
        InlineKind::Html { name, closing } => {
            assert_eq!(name, "kbd");
            assert!(closing);
        }
        other => panic!("expected html, got {other:?}"),
    }
    assert_eq!(first_inline("<3").kind, InlineKind::Text);
}

#[test]
fn markers_never_overlap_content() {
    let source = "# Title *em* **strong** `code` [a](url) ![i](s) [[N|A]] ![[e|1]] $m$ %%c%% [^f] #tag \\* <kbd> ==h== ~~s~~\n\n[^f]: body\n";
    let document = Document::parse(source);
    for block in document.blocks() {
        for inline in &block.inlines {
            assert_disjoint(inline);
        }
    }
}

fn assert_disjoint(inline: &Inline) {
    for marker in &inline.markers {
        assert!(
            marker.end <= inline.content.start || marker.start >= inline.content.end,
            "marker {marker:?} overlaps content {:?}",
            inline.content
        );
    }
    for child in &inline.children {
        assert_disjoint(child);
    }
}
