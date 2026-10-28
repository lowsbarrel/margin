use margin_markdown::{Block, BlockKind, CalloutKind, Document, Fold, HeadingStyle, Span};

fn at(blocks: &[Block], index: usize) -> &Block {
    blocks.get(index).expect("the note has this block")
}

fn only(text: &str) -> Document {
    Document::parse(text)
}

fn first(document: &Document) -> &Block {
    document.blocks().first().expect("the note has a block")
}

#[test]
fn atx_heading_ranges() {
    let source = "## Title ##\n";
    let document = only(source);
    let block = first(&document);
    assert_eq!(block.span, Span::new(0, 11));
    assert_eq!(
        block.kind,
        BlockKind::Heading {
            level: 2,
            style: HeadingStyle::Atx
        }
    );
    assert_eq!(block.markers, vec![Span::new(0, 3), Span::new(8, 11)]);
    assert_eq!(block.content.slice(source), "Title");
    assert_eq!(block.inlines.len(), 1);
}

#[test]
fn atx_heading_without_space_is_a_paragraph() {
    let document = only("#nope\n");
    assert_eq!(first(&document).kind, BlockKind::Paragraph);
}

#[test]
fn setext_heading_ranges() {
    let source = "Title\n=====\n";
    let document = only(source);
    let block = first(&document);
    assert_eq!(
        block.kind,
        BlockKind::Heading {
            level: 1,
            style: HeadingStyle::Setext
        }
    );
    assert_eq!(block.content.slice(source), "Title");
    assert_eq!(block.markers, vec![Span::new(6, 11)]);
    assert_eq!(block.span, Span::new(0, 11));
}

#[test]
fn paragraph_keeps_soft_breaks() {
    let source = "one\ntwo\n";
    let document = only(source);
    let block = first(&document);
    assert_eq!(block.kind, BlockKind::Paragraph);
    assert_eq!(block.span, Span::new(0, 7));
    assert_eq!(block.inlines.len(), 3);
}

#[test]
fn thematic_breaks() {
    for source in ["---\n", "***\n", "_ _ _\n", "  - - -\n"] {
        let document = only(source);
        assert_eq!(first(&document).kind, BlockKind::ThematicBreak, "{source}");
    }
    assert_eq!(first(&only("--\n")).kind, BlockKind::Paragraph);
}

#[test]
fn fenced_code_ranges() {
    let source = "```rust\nlet x = 1;\n```\n";
    let document = only(source);
    let block = first(&document);
    match &block.kind {
        BlockKind::CodeBlock { info, fenced } => {
            assert_eq!(info, "rust");
            assert!(*fenced);
        }
        other => panic!("expected a code block, got {other:?}"),
    }
    assert_eq!(block.language(), Some("rust"));
    assert_eq!(block.markers, vec![Span::new(0, 7), Span::new(19, 22)]);
    assert_eq!(block.content.slice(source), "let x = 1;\n");
}

#[test]
fn unterminated_fence_runs_to_the_end() {
    let source = "~~~\nbody\n";
    let document = only(source);
    let block = first(&document);
    assert!(block.is_code_block());
    assert_eq!(block.markers, vec![Span::new(0, 3)]);
    assert_eq!(block.content.slice(source), "body\n");
}

#[test]
fn indented_code() {
    let source = "    code\n    more\n\nafter\n";
    let document = only(source);
    assert!(first(&document).is_code_block());
    assert_eq!(document.blocks().len(), 2);
    assert_eq!(first(&document).content.slice(source), "    code\n    more");
}

#[test]
fn blockquote_ranges_and_nesting() {
    let source = "> one\n> > two\n";
    let document = only(source);
    let block = first(&document);
    assert_eq!(block.kind, BlockKind::BlockQuote);
    assert_eq!(block.markers, vec![Span::new(0, 2), Span::new(6, 8)]);
    assert_eq!(block.span, Span::new(0, 13));
    assert_eq!(at(&block.children, 0).content.slice(source), "one");
    let inner = at(&block.children, 1);
    assert_eq!(inner.kind, BlockKind::BlockQuote);
    assert_eq!(inner.markers, vec![Span::new(8, 10)]);
    assert_eq!(at(&inner.children, 0).content.slice(source), "two");
}

#[test]
fn callout_header_ranges() {
    let source = "> [!warning]+ Careful\n> body\n";
    let document = only(source);
    let block = first(&document);
    match &block.kind {
        BlockKind::Callout { kind, fold, title } => {
            assert_eq!(*kind, CalloutKind::Warning);
            assert_eq!(*fold, Fold::Expanded);
            assert_eq!(title, "Careful");
        }
        other => panic!("expected a callout, got {other:?}"),
    }
    assert_eq!(
        block.markers,
        vec![Span::new(0, 2), Span::new(22, 24), Span::new(2, 13)]
    );
    assert_eq!(at(&block.children, 0).content.slice(source), "body");
}

#[test]
fn callout_without_title_uses_the_type_name() {
    let source = "> [!summary]\n> body\n";
    let document = only(source);
    match &first(&document).kind {
        BlockKind::Callout { kind, title, .. } => {
            assert_eq!(*kind, CalloutKind::Abstract);
            assert_eq!(title, "Summary");
        }
        other => panic!("expected a callout, got {other:?}"),
    }
}

#[test]
fn colon_callout_ranges() {
    let source = ":::tip\ninside\n:::\n";
    let document = only(source);
    let block = first(&document);
    match &block.kind {
        BlockKind::ColonCallout { kind, name, title } => {
            assert_eq!(*kind, CalloutKind::Tip);
            assert_eq!(name, "tip");
            assert_eq!(title, "Tip");
        }
        other => panic!("expected a colon callout, got {other:?}"),
    }
    assert_eq!(block.markers, vec![Span::new(0, 6), Span::new(14, 17)]);
    assert_eq!(at(&block.children, 0).content.slice(source), "inside");
}
