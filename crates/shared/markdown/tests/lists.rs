use margin_markdown::{Block, BlockKind, Document, LineKind, Span, as_list, kind_of};

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
fn bullet_list_items_and_markers() {
    let source = "- one\n- two\n";
    let document = only(source);
    let list = first(&document);
    assert_eq!(list.kind, BlockKind::BulletList);
    assert_eq!(list.children.len(), 2);
    assert_eq!(at(&list.children, 0).markers, vec![Span::new(0, 2)]);
    assert_eq!(at(&list.children, 1).markers, vec![Span::new(6, 8)]);
    assert_eq!(at(&list.children, 0).span, Span::new(0, 5));
}

#[test]
fn ordered_list_start_and_delimiter() {
    let source = "3. three\n4. four\n";
    let document = only(source);
    match &first(&document).kind {
        BlockKind::OrderedList { start } => assert_eq!(*start, 3),
        other => panic!("expected an ordered list, got {other:?}"),
    }
    assert_eq!(
        at(&first(&document).children, 1).markers,
        vec![Span::new(9, 12)]
    );
}

#[test]
fn task_list_box_marker() {
    let source = "- [x] done\n- [ ] todo\n";
    let document = only(source);
    let list = first(&document);
    assert_eq!(at(&list.children, 0).task_state(), Some(true));
    assert_eq!(at(&list.children, 1).task_state(), Some(false));
    assert_eq!(
        at(&list.children, 0).markers,
        vec![Span::new(0, 2), Span::new(2, 5)]
    );
    assert_eq!(at(&list.children, 0).children.len(), 1);
    assert_eq!(
        at(&at(&list.children, 0).children, 0).content.slice(source),
        "done"
    );
}

#[test]
fn nested_list_inside_item() {
    let source = "- a\n  - b\n";
    let document = only(source);
    let list = first(&document);
    let item = at(&list.children, 0);
    assert_eq!(item.children.len(), 2);
    assert_eq!(at(&item.children, 0).kind, BlockKind::Paragraph);
    assert_eq!(at(&item.children, 1).kind, BlockKind::BulletList);
}

#[test]
fn loose_list_is_marked() {
    let source = "- a\n\n- b\n";
    let document = only(source);
    let list = first(&document);
    assert!(matches!(
        at(&list.children, 0).kind,
        BlockKind::ListItem { loose: true, .. }
    ));
}

#[test]
fn table_model_and_alignment() {
    let source = "| a | b |\n| --- | :---: |\n| 1 | 2 |\n";
    let document = only(source);
    let block = first(&document);
    let table = block.table().expect("the first block is a table");
    assert_eq!(table.cols, 2);
    assert_eq!(table.align.len(), 2);
    assert!(table.anchored);
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows.first().expect("a header row").cells.len(), 2);
    let header = table.rows.first().expect("a header row");
    assert_eq!(header.cells.first().expect("a cell").text, "a");
    assert_eq!(header.cells.first().expect("a cell").span, Span::new(2, 3));
    assert_eq!(header.cells.first().expect("a cell").slot, Span::new(1, 4));
    assert_eq!(block.span, Span::new(0, 35));
    assert_eq!(table.delimiter, Span::new(10, 25));
    assert_eq!(
        table
            .rows
            .get(1)
            .expect("a body row")
            .cells
            .first()
            .expect("a cell")
            .text,
        "1"
    );
}

#[test]
fn table_after_a_blank_line_is_anchored() {
    let source = "text\n\n| a |\n| - |\n";
    let document = only(source);
    let tables = document.tables();
    assert_eq!(tables.len(), 1);
    assert!(tables.first().expect("a table").anchored);
}

#[test]
fn table_after_a_block_is_not_anchored() {
    let source = "```\ncode\n```\n| a |\n| - |\n";
    let document = only(source);
    let tables = document.tables();
    assert_eq!(tables.len(), 1);
    assert!(!tables.first().expect("a table").anchored);
}

#[test]
fn block_math_single_line_and_fence() {
    let single = "$$x^2$$\n";
    let document = only(single);
    let block = first(&document);
    assert_eq!(block.kind, BlockKind::MathBlock { single_line: true });
    assert_eq!(block.content.slice(single), "x^2");
    assert_eq!(block.markers, vec![Span::new(0, 2), Span::new(5, 7)]);

    let fenced = "$$\nx^2\n$$\n";
    let document = only(fenced);
    let block = first(&document);
    assert_eq!(block.kind, BlockKind::MathBlock { single_line: false });
    assert_eq!(block.content.slice(fenced), "x^2\n");
    assert_eq!(block.markers, vec![Span::new(0, 2), Span::new(7, 9)]);
}

#[test]
fn comment_block_ranges() {
    let source = "%%\nhidden\n%%\n";
    let document = only(source);
    let block = first(&document);
    assert_eq!(block.kind, BlockKind::Comment);
    assert_eq!(block.markers, vec![Span::new(0, 2), Span::new(10, 12)]);
    assert_eq!(block.content.slice(source), "\nhidden\n");
}

#[test]
fn footnote_definition_and_numbering() {
    let source = "text[^a] more[^b]\n\n[^a]: first\n[^b]: second\n";
    let document = only(source);
    let footnotes = document.footnotes();
    assert_eq!(footnotes.definitions.len(), 2);
    assert_eq!(footnotes.number("a"), Some(1));
    assert_eq!(footnotes.number("b"), Some(2));
    assert_eq!(footnotes.references.len(), 2);
    let definition = footnotes.definition("a").expect("a definition");
    assert_eq!(definition.span.slice(source), "[^a]: first");
    assert_eq!(definition.body.slice(source), "first");
}

#[test]
fn footnote_reference_markers() {
    let source = "[^a]: body\n\nsee[^a]\n";
    let document = only(source);
    let paragraph = at(document.blocks(), 1);
    let reference = paragraph
        .inlines
        .iter()
        .find(|inline| {
            matches!(
                inline.kind,
                margin_markdown::InlineKind::FootnoteReference { .. }
            )
        })
        .expect("a footnote reference");
    assert_eq!(
        reference.markers,
        vec![Span::new(15, 17), Span::new(18, 19)]
    );
    assert_eq!(reference.content.slice(source), "a");
}

#[test]
fn frontmatter_block_and_properties() {
    let source = "---\ntitle: Hello\ntags:\n  - a\n  - b\n---\nbody\n";
    let document = only(source);
    let frontmatter = document.frontmatter().expect("frontmatter");
    assert_eq!(frontmatter.open, Span::new(0, 3));
    assert_eq!(
        frontmatter.span.slice(source),
        "---\ntitle: Hello\ntags:\n  - a\n  - b\n---"
    );
    assert!(!frontmatter.unsupported);
    let title = frontmatter.property("title").expect("title");
    assert_eq!(
        title.value,
        margin_markdown::PropertyValue::Text("Hello".to_owned())
    );
    assert_eq!(title.span.slice(source), "title: Hello");
    let tags = frontmatter.property("tags").expect("tags");
    assert_eq!(as_list(&tags.value), vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(
        kind_of("tags", &tags.value, None),
        margin_markdown::PropertyKind::Tags
    );
    assert_eq!(at(document.blocks(), 1).content.slice(source), "body");
}

#[test]
fn frontmatter_must_start_the_document() {
    let document = only("text\n---\n");
    assert!(document.frontmatter().is_none());
}

#[test]
fn html_block_is_one_block() {
    let source = "<div class=\"x\">\nraw\n</div>\n\nafter\n";
    let document = only(source);
    assert_eq!(first(&document).kind, BlockKind::HtmlBlock);
    assert_eq!(
        first(&document).span.slice(source),
        "<div class=\"x\">\nraw\n</div>"
    );
    assert_eq!(document.blocks().len(), 2);
}

#[test]
fn link_reference_definition_is_not_rendered() {
    let source = "[a]: https://x \"t\"\n\n[a]\n";
    let document = only(source);
    assert!(matches!(
        first(&document).kind,
        BlockKind::LinkReference { .. }
    ));
    assert_eq!(
        document
            .definitions()
            .get("a")
            .expect("a definition")
            .destination,
        "https://x"
    );
    assert_eq!(document.blocks().len(), 2);
}

#[test]
fn line_kinds_are_reported_per_line() {
    let source = "# h\n\ntext\n\n- item\n\n> quote\n\n```\ncode\n```\n";
    let document = only(source);
    assert_eq!(document.line_count(), 12);
    assert_eq!(document.line_kind(0), LineKind::Heading);
    assert_eq!(document.line_kind(1), LineKind::Blank);
    assert_eq!(document.line_kind(2), LineKind::Paragraph);
    assert_eq!(document.line_kind(4), LineKind::ListItem);
    assert_eq!(document.line_kind(6), LineKind::BlockQuote);
    assert_eq!(document.line_kind(9), LineKind::Code);
    assert_eq!(document.line_text(2), "text");
    assert_eq!(document.line_of(13), 4);
    assert_eq!(document.line_span(2), Span::new(5, 9));
}

#[test]
fn block_at_descends_into_containers() {
    let source = "> - item\n";
    let document = only(source);
    assert!(matches!(
        document.block_at(2).expect("a block at the marker").kind,
        BlockKind::ListItem { .. }
    ));
    assert_eq!(
        document.block_at(6).expect("a block in the text").kind,
        BlockKind::Paragraph
    );
}

#[test]
fn long_note_parses_every_line() {
    let mut source = String::new();
    for index in 0..10_000 {
        source.push_str("line ");
        source.push_str(&index.to_string());
        source.push_str("\n\n");
    }
    let document = Document::parse(&source);
    assert_eq!(document.line_count(), 20_001);
    assert_eq!(document.blocks().len(), 10_000);
    assert_eq!(document.line_kind(0), LineKind::Paragraph);
}
