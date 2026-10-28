use std::cmp::Ordering;

use margin_markdown::{
    Document, Frontmatter, Property, PropertyKind, PropertyValue, as_list, kind_of,
    serialize_property,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SortOrder {
    Ascending,
    Descending,
}

pub(crate) fn frontmatter_of(source: &str) -> Option<Frontmatter> {
    let mut lines = source.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let mut end = first.len();
    for line in lines {
        end += line.len();
        let text = line.trim_end();
        if text == "---" || text == "..." {
            let prefix = source.get(..end)?;
            return Document::parse(prefix).frontmatter().cloned();
        }
    }
    None
}

fn supported(source: &str) -> Option<Frontmatter> {
    frontmatter_of(source).filter(|frontmatter| !frontmatter.unsupported)
}

fn changed(source: &str, next: String) -> Option<String> {
    (next != source).then_some(next)
}

fn past_line_break(source: &str, at: usize) -> usize {
    let rest = source.get(at..).unwrap_or_default();
    if rest.starts_with("\r\n") {
        at + 2
    } else if rest.starts_with('\n') {
        at + 1
    } else {
        at
    }
}

pub(crate) fn set_value(source: &str, key: &str, value: &PropertyValue) -> Option<String> {
    let frontmatter = supported(source)?;
    frontmatter.property(key)?;
    changed(source, frontmatter.set_property(source, key, value))
}

pub(crate) fn rename(source: &str, key: &str, next: &str) -> Option<String> {
    let frontmatter = supported(source)?;
    frontmatter.property(key)?;
    changed(source, frontmatter.rename_property(source, key, next))
}

pub(crate) fn add(source: &str, key: &str) -> Option<String> {
    match frontmatter_of(source) {
        Some(frontmatter) if frontmatter.unsupported => None,
        Some(frontmatter) => changed(source, frontmatter.add_property(source, key)),
        None => {
            let line = serialize_property(key, &PropertyValue::Null);
            Some(format!("---\n{line}\n---\n{source}"))
        }
    }
}

pub(crate) fn remove(source: &str, keys: &[&str]) -> Option<String> {
    let frontmatter = supported(source)?;
    let (gone, left): (Vec<&Property>, Vec<&Property>) = frontmatter
        .properties
        .iter()
        .partition(|entry| keys.contains(&entry.key.as_str()));
    if gone.is_empty() {
        return None;
    }
    if left.is_empty() {
        let body = past_line_break(source, frontmatter.end());
        return Some(source.get(body..).unwrap_or_default().to_owned());
    }
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for entry in gone {
        out.push_str(source.get(at..entry.span.start).unwrap_or_default());
        at = past_line_break(source, entry.span.end);
    }
    out.push_str(source.get(at..).unwrap_or_default());
    Some(out)
}

pub(crate) fn sort(source: &str, order: SortOrder) -> Option<String> {
    let frontmatter = supported(source)?;
    let [first, .., last] = frontmatter.properties.as_slice() else {
        return None;
    };
    let mut sorted: Vec<&Property> = frontmatter.properties.iter().collect();
    sorted.sort_by(|left, right| match order {
        SortOrder::Ascending => compare_keys(&left.key, &right.key),
        SortOrder::Descending => compare_keys(&right.key, &left.key),
    });
    let block = sorted
        .iter()
        .map(|entry| entry.span.slice(source))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = source
        .get(..first.span.start)
        .unwrap_or_default()
        .to_owned();
    out.push_str(&block);
    out.push_str(source.get(last.span.end..).unwrap_or_default());
    changed(source, out)
}

fn compare_keys(left: &str, right: &str) -> Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

pub(crate) fn convert_value(value: &PropertyValue, kind: PropertyKind) -> Option<PropertyValue> {
    if *value == PropertyValue::Null {
        return Some(PropertyValue::Null);
    }
    if kind.is_list() {
        return Some(PropertyValue::List(as_list(value)));
    }
    if let PropertyValue::List(items) = value {
        return (kind == PropertyKind::Text).then(|| PropertyValue::Text(items.join(", ")));
    }
    let text = value.to_plain_string();
    match kind {
        PropertyKind::Text => Some(PropertyValue::Text(text)),
        PropertyKind::Number => parse_number(&text).map(PropertyValue::Number),
        PropertyKind::Checkbox => match text.as_str() {
            "true" => Some(PropertyValue::Bool(true)),
            "false" => Some(PropertyValue::Bool(false)),
            _ => None,
        },
        PropertyKind::Date | PropertyKind::Datetime => {
            let (date, time) = date_parts(&text)?;
            Some(PropertyValue::Text(if kind == PropertyKind::Date {
                date.to_owned()
            } else {
                format!("{date}T{}", time.unwrap_or("00:00:00"))
            }))
        }
        PropertyKind::List | PropertyKind::Tags | PropertyKind::Aliases | PropertyKind::Raw => None,
    }
}

pub(crate) fn parse_number(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

pub(crate) fn date_parts(text: &str) -> Option<(&str, Option<&str>)> {
    let probe = PropertyValue::Text(text.to_owned());
    match kind_of("", &probe, None) {
        PropertyKind::Date => Some((text, None)),
        PropertyKind::Datetime => text.split_once('T').map(|(date, time)| (date, Some(time))),
        _ => None,
    }
}

pub(crate) fn change_kind(source: &str, key: &str, kind: PropertyKind) -> Option<String> {
    let frontmatter = supported(source)?;
    let property = frontmatter.property(key)?;
    let value = convert_value(&property.value, kind)?;
    if value == property.value {
        return Some(source.to_owned());
    }
    Some(frontmatter.set_property(source, key, &value))
}

pub(crate) fn effective_kind(property: &Property, chosen: Option<PropertyKind>) -> PropertyKind {
    let inferred = kind_of(&property.key, &property.value, property.raw.as_deref());
    let fixed = matches!(
        inferred,
        PropertyKind::Raw | PropertyKind::Tags | PropertyKind::Aliases
    );
    match chosen {
        Some(kind)
            if !fixed && convert_value(&property.value, kind).as_ref() == Some(&property.value) =>
        {
            kind
        }
        _ => inferred,
    }
}

pub(crate) fn key_problem(source: &str, key: &str, except: Option<&str>) -> Option<&'static str> {
    if key.is_empty() {
        return Some("editor_property_name_empty");
    }
    let lower = key.to_lowercase();
    let taken = supported(source).is_some_and(|frontmatter| {
        frontmatter
            .properties
            .iter()
            .any(|entry| Some(entry.key.as_str()) != except && entry.key.to_lowercase() == lower)
    });
    taken.then_some("editor_property_name_taken")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str =
        "---\ntitle: Roadmap\ntags:\n  - plan\n  - q3\ndone: false\n---\n# Body\n\nText ---\n";
    const BODY: &str = "# Body\n\nText ---\n";

    fn body_of(source: &str) -> &str {
        let end = frontmatter_of(source)
            .expect("the edit keeps the frontmatter")
            .end();
        &source[past_line_break(source, end)..]
    }

    #[test]
    fn setting_a_value_rewrites_only_its_line() {
        let next = set_value(NOTE, "title", &PropertyValue::Text("Launch".to_owned()))
            .expect("the value changed");
        assert!(next.starts_with("---\ntitle: Launch\ntags:\n"));
        assert_eq!(body_of(&next), BODY);
    }

    #[test]
    fn setting_the_same_value_changes_nothing() {
        let same = PropertyValue::Text("Roadmap".to_owned());
        assert_eq!(set_value(NOTE, "title", &same), None);
    }

    #[test]
    fn renaming_keeps_the_value_and_the_body() {
        let next = rename(NOTE, "done", "finished").expect("the key changed");
        assert!(next.contains("\nfinished: false\n---\n"));
        assert_eq!(body_of(&next), BODY);
    }

    #[test]
    fn adding_appends_before_the_closing_fence() {
        let next = add(NOTE, "owner").expect("a property was added");
        assert!(next.contains("done: false\nowner:\n---\n"));
        assert_eq!(body_of(&next), BODY);
    }

    #[test]
    fn adding_to_a_note_without_frontmatter_creates_it() {
        assert_eq!(
            add(BODY, "owner"),
            Some(format!("---\nowner:\n---\n{BODY}"))
        );
    }

    #[test]
    fn removing_drops_the_whole_entry() {
        let next = remove(NOTE, &["tags"]).expect("a property was removed");
        assert_eq!(
            next,
            format!("---\ntitle: Roadmap\ndone: false\n---\n{BODY}")
        );
    }

    #[test]
    fn removing_every_property_drops_the_frontmatter() {
        assert_eq!(
            remove(NOTE, &["title", "tags", "done"]),
            Some(BODY.to_owned())
        );
    }

    #[test]
    fn list_items_are_added_and_removed() {
        let added = PropertyValue::List(vec!["plan".to_owned(), "q3".to_owned(), "ops".to_owned()]);
        let next = set_value(NOTE, "tags", &added).expect("an item was added");
        assert!(next.contains("tags:\n  - plan\n  - q3\n  - ops\ndone"));
        assert_eq!(body_of(&next), BODY);
        let removed = PropertyValue::List(vec!["q3".to_owned()]);
        let next = set_value(&next, "tags", &removed).expect("items were removed");
        assert!(next.contains("tags:\n  - q3\ndone"));
        assert_eq!(body_of(&next), BODY);
    }

    #[test]
    fn sorting_reorders_entries() {
        let next = sort(NOTE, SortOrder::Ascending).expect("the order changed");
        assert!(
            next.starts_with("---\ndone: false\ntags:\n  - plan\n  - q3\ntitle: Roadmap\n---\n")
        );
        assert_eq!(body_of(&next), BODY);
    }

    #[test]
    fn kinds_convert_only_when_the_value_fits() {
        let text = PropertyValue::Text("2024-05-01T10:30".to_owned());
        assert_eq!(
            convert_value(&text, PropertyKind::Date),
            Some(PropertyValue::Text("2024-05-01".to_owned()))
        );
        assert_eq!(convert_value(&text, PropertyKind::Number), None);
        assert_eq!(
            convert_value(&PropertyValue::Text("12".to_owned()), PropertyKind::Number),
            Some(PropertyValue::Number(12.0))
        );
    }

    #[test]
    fn taken_keys_ignore_case_and_the_key_itself() {
        assert_eq!(
            key_problem(NOTE, "Title", None),
            Some("editor_property_name_taken")
        );
        assert_eq!(key_problem(NOTE, "title", Some("title")), None);
        assert_eq!(
            key_problem(NOTE, "", None),
            Some("editor_property_name_empty")
        );
    }
}
