use margin_markdown::{
    Align, CellPlace, Document, Span, align_of, display_text, empty_row, format_table, is_wide,
    rewrite, width_of,
};

#[test]
fn alignment_parsing() {
    assert_eq!(align_of("---"), Align::None);
    assert_eq!(align_of(":-"), Align::Left);
    assert_eq!(align_of("-:"), Align::Right);
    assert_eq!(align_of(":-:"), Align::Center);
    assert_eq!(align_of(":--:"), Align::Center);
    assert_eq!(align_of(" x "), Align::None);
    assert_eq!(align_of(":"), Align::None);
}

#[test]
fn display_text_unescapes_pipes() {
    assert_eq!(display_text("a \\| b"), "a | b");
    assert_eq!(display_text("a \\\\ b"), "a \\ b");
    assert_eq!(display_text("plain"), "plain");
}

#[test]
fn width_counts_wide_characters() {
    assert_eq!(width_of("abc"), 3);
    assert_eq!(width_of("日本"), 4);
    assert!(is_wide('日'));
    assert!(!is_wide('a'));
}

#[test]
fn empty_row_has_the_requested_columns() {
    assert_eq!(empty_row(3), vec![String::new(); 3]);
}

#[test]
fn format_table_pads_and_aligns() {
    let rows = vec![
        vec!["a".to_owned(), "b".to_owned()],
        vec!["1".to_owned(), "2".to_owned()],
    ];
    let formatted = format_table(&rows, &[Align::Left, Align::Right]);
    assert_eq!(
        formatted.lines,
        vec![
            "| a   |   b |".to_owned(),
            "| :-- | --: |".to_owned(),
            "| 1   |   2 |".to_owned()
        ]
    );
    assert_eq!(formatted.starts, vec![vec![2, 10], vec![2, 10]]);
    let header = formatted.lines.first().expect("a header line");
    assert_eq!(header.get(2..3), Some("a"));
    assert_eq!(header.get(10..11), Some("b"));
}

#[test]
fn format_table_centers_and_keeps_minimum_width() {
    let rows = vec![vec!["x".to_owned()]];
    let formatted = format_table(&rows, &[Align::Center]);
    assert_eq!(formatted.lines.first().expect("a line"), "|  x  |");
    assert_eq!(formatted.lines.get(1).expect("a rule"), "| :-: |");
    assert_eq!(formatted.starts.first().expect("starts"), &vec![3]);
}

#[test]
fn rewrite_reproduces_the_table_source() {
    let source = "| a | b |\n| --- | --- |\n| 1 | 2 |\n";
    let document = Document::parse(source);
    let table = document.tables().into_iter().next().expect("a table");
    let rows = table.text_rows();
    let rewritten = rewrite(table, source, &rows, &table.align);
    assert_eq!(
        rewritten.text,
        "| a   | b   |\n| --- | --- |\n| 1   | 2   |"
    );
    assert_eq!(rewritten.cell_starts, vec![vec![2, 8], vec![30, 36]]);
    let first = rewritten.cell_starts.first().expect("starts");
    assert_eq!(
        rewritten.text.get(first[0]..first[0] + 1),
        Some("a"),
        "the cell start must address the cell text"
    );
}

#[test]
fn rewrite_keeps_indentation() {
    let source = "  | a |\n  | - |\n  | 1 |\n";
    let document = Document::parse(source);
    let table = document.tables().into_iter().next().expect("a table");
    let rows = table.text_rows();
    let rewritten = rewrite(table, source, &rows, &table.align);
    assert_eq!(rewritten.text, "  | a   |\n  | --- |\n  | 1   |");
    assert_eq!(rewritten.cell_starts.first().expect("starts"), &vec![4]);
}

#[test]
fn rewrite_of_a_nested_table_starts_at_its_own_offset() {
    let source = "- | a |\n  | - |\n  | 1 |\n";
    let document = Document::parse(source);
    let table = document.tables().into_iter().next().expect("a table");
    assert_eq!(table.span.start, 2);
    let rows = table.text_rows();
    let rewritten = rewrite(table, source, &rows, &table.align);
    assert_eq!(rewritten.text, "| a   |\n  | --- |\n  | 1   |");
}

#[test]
fn locate_cell_finds_the_column() {
    let source = "| a | b |\n| - | - |\n| 1 | 2 |\n";
    let document = Document::parse(source);
    let table = document.tables().into_iter().next().expect("a table");
    assert_eq!(table.locate_cell(2), Some(CellPlace { row: 0, col: 0 }));
    assert_eq!(table.locate_cell(6), Some(CellPlace { row: 0, col: 1 }));
    assert_eq!(table.locate_cell(28), Some(CellPlace { row: 1, col: 1 }));
    assert_eq!(table.locate_cell(1000), None);
    assert_eq!(
        table
            .cell(CellPlace { row: 1, col: 1 })
            .map(|cell| cell.text.as_str()),
        Some("2")
    );
    assert_eq!(table.span, Span::new(0, 29));
}

#[test]
fn escaped_pipes_stay_inside_one_cell() {
    let source = "| a \\| b | c |\n| - | - |\n";
    let document = Document::parse(source);
    let table = document.tables().into_iter().next().expect("a table");
    assert_eq!(table.cols, 2);
    let header = table.rows.first().expect("a header row");
    assert_eq!(header.cells.first().expect("a cell").text, "a \\| b");
    assert_eq!(
        display_text(&header.cells.first().expect("a cell").text),
        "a | b"
    );
}

#[test]
fn rows_are_padded_to_the_column_count() {
    let source = "| a | b | c |\n| - | - | - |\n| 1 |\n";
    let document = Document::parse(source);
    let table = document.tables().into_iter().next().expect("a table");
    assert_eq!(table.cols, 3);
    let body = table.rows.get(1).expect("a body row");
    assert_eq!(body.cells.len(), 3);
    assert_eq!(body.cells.get(2).expect("a padded cell").text, "");
}
