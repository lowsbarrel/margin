use margin_markdown::{Document, Span, TextEdit};

fn assert_matches_full_parse(previous: &Document, edit: &TextEdit) {
    let edited = previous.reparse(edit);
    let full = Document::parse(edited.text());
    assert_eq!(
        edited.blocks(),
        full.blocks(),
        "a reparse of {edit:?} must equal a full parse"
    );
    assert_eq!(edited.line_count(), full.line_count());
    assert_eq!(edited.footnotes(), full.footnotes());
}

fn replace(previous: &Document, start: usize, end: usize, insert: &str) {
    assert_matches_full_parse(previous, &TextEdit::new(Span::new(start, end), insert));
}

const NOTE: &str = "# Title\n\nfirst paragraph\n\n- one\n- two\n\n> quote\n\n```rust\nlet x = 1;\n```\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\nsee[^n]\n\n[^n]: note body\n";

#[test]
fn typing_inside_a_paragraph() {
    let document = Document::parse(NOTE);
    replace(&document, 13, 13, "X");
    replace(&document, 13, 18, "");
    replace(&document, 12, 13, "\n\n");
}

#[test]
fn typing_a_heading_marker() {
    let document = Document::parse(NOTE);
    replace(&document, 13, 13, "## ");
}

#[test]
fn joining_and_splitting_blocks() {
    let document = Document::parse("one\n\ntwo\n");
    replace(&document, 3, 5, "");
    replace(&document, 3, 3, "\n\n");
}

#[test]
fn editing_a_list_item() {
    let document = Document::parse(NOTE);
    replace(&document, 28, 28, "!");
    replace(&document, 27, 32, "");
}

#[test]
fn editing_a_fence() {
    let document = Document::parse(NOTE);
    replace(&document, 55, 55, "let y = 2;\n");
    replace(&document, 45, 48, "```");
    replace(&document, 44, 44, "\n");
}

#[test]
fn editing_a_table_cell() {
    let document = Document::parse(NOTE);
    replace(&document, 74, 75, "9");
    replace(&document, 70, 70, "  ");
    replace(&document, 80, 81, "\n");
}

#[test]
fn editing_a_quote_and_a_callout() {
    let document = Document::parse("> [!note] Title\n> body\n");
    replace(&document, 9, 14, "Summary");
    replace(&document, 15, 15, "\n> more");
    replace(&document, 0, 2, "");
}

#[test]
fn editing_a_footnote_definition() {
    let document = Document::parse(NOTE);
    let start = NOTE.find("note body").expect("the definition body");
    replace(&document, start, start + 4, "word");
    replace(&document, start, start + 9, "");
}

#[test]
fn editing_a_link_reference_definition() {
    let document = Document::parse("[a][b]\n\n[b]: url\n");
    replace(&document, 9, 12, "other");
    replace(&document, 4, 9, "");
}

#[test]
fn appending_and_prepending() {
    let document = Document::parse("body\n");
    replace(&document, 5, 5, "\n# tail\n");
    replace(&document, 0, 0, "# head\n\n");
    replace(&document, 0, 0, "---\ntitle: T\n---\n");
}

#[test]
fn replacing_the_whole_document() {
    let document = Document::parse(NOTE);
    replace(&document, 0, NOTE.len(), "just one line\n");
    replace(&document, 0, 0, "prefix\n");
}

#[test]
fn reparse_of_a_long_note_matches_a_full_parse() {
    let mut source = String::new();
    for index in 0..10_000 {
        source.push_str("paragraph ");
        source.push_str(&index.to_string());
        source.push_str("\n\n");
    }
    let document = Document::parse(&source);
    assert_eq!(document.blocks().len(), 10_000);
    let offset = source.find("paragraph 5000").expect("the middle paragraph");
    let edited = document.reparse(&TextEdit::new(Span::new(offset + 10, offset + 14), "x"));
    let full = Document::parse(edited.text());
    assert_eq!(edited.blocks(), full.blocks());
    assert_eq!(edited.blocks().len(), 10_000);
}
