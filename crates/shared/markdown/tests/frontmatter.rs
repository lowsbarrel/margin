use margin_markdown::{
    Document, PropertyKind, PropertyValue, Span, as_list, kind_of, rename_source,
    serialize_property,
};

fn property(source: &str, key: &str) -> margin_markdown::Property {
    Document::parse(source)
        .frontmatter()
        .expect("frontmatter")
        .property(key)
        .expect("the property")
        .clone()
}

#[test]
fn scalar_kinds_are_inferred() {
    let source = "---\ntext: hello\nnumber: 3\nfloat: 1.5\ncheckbox: true\ndate: 2024-05-06\ndatetime: 2024-05-06T07:08\ntime: 2024-05-06T07:08:09\nempty:\n---\n";
    assert_eq!(
        property(source, "text").value,
        PropertyValue::Text("hello".to_owned())
    );
    assert_eq!(property(source, "number").value, PropertyValue::Number(3.0));
    assert_eq!(property(source, "float").value, PropertyValue::Number(1.5));
    assert_eq!(
        property(source, "checkbox").value,
        PropertyValue::Bool(true)
    );
    assert_eq!(property(source, "empty").value, PropertyValue::Null);
    assert_eq!(
        kind_of("number", &PropertyValue::Number(3.0), None),
        PropertyKind::Number
    );
    assert_eq!(
        kind_of("checkbox", &PropertyValue::Bool(false), None),
        PropertyKind::Checkbox
    );
    assert_eq!(
        kind_of("date", &PropertyValue::Text("2024-05-06".to_owned()), None),
        PropertyKind::Date
    );
    assert_eq!(
        kind_of(
            "datetime",
            &PropertyValue::Text("2024-05-06T07:08".to_owned()),
            None
        ),
        PropertyKind::Datetime
    );
    assert_eq!(
        kind_of("when", &PropertyValue::Text("2024-5-6".to_owned()), None),
        PropertyKind::Text
    );
}

#[test]
fn quoted_and_plain_scalars() {
    let source = "---\na: \"quoted\"\nb: 'single''d'\nc: plain # comment\nd: ~\n---\n";
    assert_eq!(
        property(source, "a").value,
        PropertyValue::Text("quoted".to_owned())
    );
    assert_eq!(
        property(source, "b").value,
        PropertyValue::Text("single'd".to_owned())
    );
    assert_eq!(
        property(source, "c").value,
        PropertyValue::Text("plain".to_owned())
    );
    assert_eq!(property(source, "d").value, PropertyValue::Null);
}

#[test]
fn flow_and_block_lists() {
    let flow = "---\ntags: [a, \"b c\"]\n---\n";
    assert_eq!(
        as_list(&property(flow, "tags").value),
        vec!["a".to_owned(), "b c".to_owned()]
    );
    assert_eq!(
        kind_of("tags", &property(flow, "tags").value, None),
        PropertyKind::Tags
    );

    let block = "---\naliases:\n  - one\n  - two\n---\n";
    assert_eq!(
        as_list(&property(block, "aliases").value),
        vec!["one".to_owned(), "two".to_owned()]
    );
    assert_eq!(
        kind_of("aliases", &property(block, "aliases").value, None),
        PropertyKind::Aliases
    );
}

#[test]
fn block_scalars_are_folded_and_chomped() {
    let literal = "---\ntext: |\n  one\n  two\n---\n";
    assert_eq!(
        property(literal, "text").value,
        PropertyValue::Text("one\ntwo\n".to_owned())
    );

    let folded = "---\ntext: >-\n  one\n  two\n---\n";
    assert_eq!(
        property(folded, "text").value,
        PropertyValue::Text("one two".to_owned())
    );
}

#[test]
fn unsupported_values_stay_raw() {
    let source = "---\nnested:\n  child: 1\n---\n";
    let entry = property(source, "nested");
    assert_eq!(entry.raw.as_deref(), Some("nested:\n  child: 1"));
    assert_eq!(
        kind_of("nested", &entry.value, entry.raw.as_deref()),
        PropertyKind::Raw
    );
}

#[test]
fn invalid_yaml_marks_the_block_unsupported() {
    let document = Document::parse("---\nkey:value\n---\n");
    let frontmatter = document.frontmatter().expect("frontmatter");
    assert!(frontmatter.unsupported);
    assert!(frontmatter.properties.is_empty());
    assert_eq!(
        frontmatter.span.slice(document.text()),
        "---\nkey:value\n---"
    );
}

#[test]
fn property_spans_cover_the_source_lines() {
    let source = "---\ntitle: Hello\n---\n";
    let entry = property(source, "title");
    assert_eq!(entry.span, Span::new(4, 16));
    assert_eq!(entry.span.slice(source), "title: Hello");
}

#[test]
fn serialize_property_writes_obsidian_yaml() {
    assert_eq!(
        serialize_property("title", &PropertyValue::Text("Hello".to_owned())),
        "title: Hello"
    );
    assert_eq!(
        serialize_property("count", &PropertyValue::Number(3.0)),
        "count: 3"
    );
    assert_eq!(
        serialize_property("done", &PropertyValue::Bool(true)),
        "done: true"
    );
    assert_eq!(serialize_property("empty", &PropertyValue::Null), "empty:");
    assert_eq!(
        serialize_property("empty", &PropertyValue::Text(String::new())),
        "empty:"
    );
    assert_eq!(
        serialize_property(
            "tags",
            &PropertyValue::List(vec!["a".to_owned(), "b".to_owned()])
        ),
        "tags:\n  - a\n  - b"
    );
    assert_eq!(
        serialize_property("tags", &PropertyValue::List(Vec::new())),
        "tags:"
    );
    assert_eq!(
        serialize_property("note", &PropertyValue::Text("a: b".to_owned())),
        "note: \"a: b\""
    );
    assert_eq!(
        serialize_property("note", &PropertyValue::Text("3".to_owned())),
        "note: \"3\""
    );
    assert_eq!(
        serialize_property("note", &PropertyValue::Text("multi\nline".to_owned())),
        "note: |-\n  multi\n  line"
    );
}

#[test]
fn set_property_replaces_one_line() {
    let source = "---\ntitle: Hello\ncount: 1\n---\nbody\n";
    let frontmatter = Document::parse(source)
        .frontmatter()
        .expect("frontmatter")
        .clone();
    let updated = frontmatter.set_property(source, "title", &PropertyValue::Text("Bye".to_owned()));
    assert_eq!(updated, "---\ntitle: Bye\ncount: 1\n---\nbody\n");
    let unchanged =
        frontmatter.set_property(source, "title", &PropertyValue::Text("Hello".to_owned()));
    assert_eq!(unchanged, source);
}

#[test]
fn set_property_appends_before_the_closing_fence() {
    let source = "---\ntitle: Hello\n---\nbody\n";
    let frontmatter = Document::parse(source)
        .frontmatter()
        .expect("frontmatter")
        .clone();
    let updated =
        frontmatter.set_property(source, "tags", &PropertyValue::List(vec!["a".to_owned()]));
    assert_eq!(updated, "---\ntitle: Hello\ntags:\n  - a\n---\nbody\n");
}

#[test]
fn rename_property_keeps_the_value() {
    let source = "---\ntitle: Hello\n---\n";
    let frontmatter = Document::parse(source)
        .frontmatter()
        .expect("frontmatter")
        .clone();
    assert_eq!(
        frontmatter.rename_property(source, "title", "name"),
        "---\nname: Hello\n---\n"
    );
}

#[test]
fn rename_source_keeps_raw_values() {
    assert_eq!(rename_source("old: value", "new"), "new: value");
    assert_eq!(
        rename_source("nested:\n  child: 1", "flat"),
        "flat:\n  child: 1"
    );
}

#[test]
fn missing_frontmatter_has_no_properties() {
    let document = Document::parse("body\n");
    assert!(document.frontmatter().is_none());
}
