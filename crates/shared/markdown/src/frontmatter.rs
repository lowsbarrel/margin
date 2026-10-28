use crate::frontmatter_key::key_line;
use crate::frontmatter_write::{format_number, rename_source, serialize_property};
use crate::span::Span;
use crate::text::Line;
use crate::yaml;

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<String>),
}

impl PropertyValue {
    #[must_use]
    pub fn to_plain_string(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => format_number(*value),
            Self::Text(text) => text.clone(),
            Self::List(items) => items.join(", "),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Null => true,
            Self::Text(text) => text.is_empty(),
            Self::List(items) => items.is_empty(),
            _ => false,
        }
    }

    #[must_use]
    pub fn normalized(&self) -> Self {
        if self.is_empty() {
            Self::Null
        } else {
            self.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyKind {
    Text,
    List,
    Number,
    Checkbox,
    Date,
    Datetime,
    Tags,
    Aliases,
    Raw,
}

impl PropertyKind {
    #[must_use]
    pub const fn is_list(self) -> bool {
        matches!(self, Self::List | Self::Tags | Self::Aliases)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub key: String,
    pub value: PropertyValue,
    pub raw: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frontmatter {
    pub span: Span,
    pub open: Span,
    pub close: Span,
    pub properties: Vec<Property>,
    pub unsupported: bool,
}

impl Frontmatter {
    #[must_use]
    pub fn property(&self, key: &str) -> Option<&Property> {
        self.properties.iter().find(|entry| entry.key == key)
    }

    #[must_use]
    pub const fn end(&self) -> usize {
        self.span.end
    }

    #[must_use]
    pub fn set_property(&self, source: &str, key: &str, value: &PropertyValue) -> String {
        let next = value.normalized();
        if let Some(property) = self.property(key) {
            if property.raw.is_none() && property.value == next {
                return source.to_owned();
            }
            let mut out = Span::new(0, property.span.start).slice(source).to_owned();
            out.push_str(&serialize_property(&property.key, &next));
            out.push_str(Span::new(property.span.end, source.len()).slice(source));
            return out;
        }
        let mut insert = serialize_property(key, &next);
        insert.push('\n');
        let at = self.close.start;
        let mut out = Span::new(0, at).slice(source).to_owned();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&insert);
        out.push_str(Span::new(at, source.len()).slice(source));
        out
    }

    #[must_use]
    pub fn rename_property(&self, source: &str, key: &str, next: &str) -> String {
        let Some(property) = self.property(key) else {
            return source.to_owned();
        };
        if key == next {
            return source.to_owned();
        }
        let replacement = match &property.raw {
            Some(raw) => rename_source(raw, next),
            None => serialize_property(next, &property.value),
        };
        let mut out = Span::new(0, property.span.start).slice(source).to_owned();
        out.push_str(&replacement);
        out.push_str(Span::new(property.span.end, source.len()).slice(source));
        out
    }

    #[must_use]
    pub fn add_property(&self, source: &str, key: &str) -> String {
        self.set_property(source, key, &PropertyValue::Null)
    }
}

#[must_use]
pub fn kind_of(key: &str, value: &PropertyValue, raw: Option<&str>) -> PropertyKind {
    if raw.is_some() {
        return PropertyKind::Raw;
    }
    match key.to_lowercase().as_str() {
        "tags" => return PropertyKind::Tags,
        "aliases" => return PropertyKind::Aliases,
        "cssclasses" => return PropertyKind::List,
        _ => {}
    }
    match value {
        PropertyValue::Null => PropertyKind::Text,
        PropertyValue::List(_) => PropertyKind::List,
        PropertyValue::Number(_) => PropertyKind::Number,
        PropertyValue::Bool(_) => PropertyKind::Checkbox,
        PropertyValue::Text(text) => {
            if is_datetime(text) {
                PropertyKind::Datetime
            } else if is_date(text) {
                PropertyKind::Date
            } else {
                PropertyKind::Text
            }
        }
    }
}

#[must_use]
pub fn as_list(value: &PropertyValue) -> Vec<String> {
    match value {
        PropertyValue::Null => Vec::new(),
        PropertyValue::Text(text) if text.is_empty() => Vec::new(),
        PropertyValue::List(items) => items.clone(),
        other => vec![other.to_plain_string()],
    }
}

#[must_use]
pub(crate) fn is_date(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    match chars.as_slice() {
        [y0, y1, y2, y3, '-', m0, m1, '-', d0, d1] => {
            y0.is_ascii_digit()
                && y1.is_ascii_digit()
                && y2.is_ascii_digit()
                && y3.is_ascii_digit()
                && matches!(m0, '0' | '1')
                && m1.is_ascii_digit()
                && matches!(d0, '0'..='3')
                && d1.is_ascii_digit()
        }
        _ => false,
    }
}

fn hour(first: char, second: char) -> bool {
    matches!(first, '0'..='2') && second.is_ascii_digit()
}

fn minute(first: char, second: char) -> bool {
    matches!(first, '0'..='5') && second.is_ascii_digit()
}

#[must_use]
pub(crate) fn is_datetime(text: &str) -> bool {
    let Some((date, rest)) = text.split_once('T') else {
        return false;
    };
    if !is_date(date) {
        return false;
    }
    let chars: Vec<char> = rest.chars().collect();
    match chars.as_slice() {
        [h0, h1, ':', m0, m1] => hour(*h0, *h1) && minute(*m0, *m1),
        [h0, h1, ':', m0, m1, ':', s0, s1] => {
            hour(*h0, *h1) && minute(*m0, *m1) && minute(*s0, *s1)
        }
        _ => false,
    }
}

pub(crate) struct KeyLine<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

struct Pending {
    key: String,
    inline: String,
    from: usize,
    to: usize,
    rest: Vec<String>,
}

fn is_chomp_indented(inline: &str) -> bool {
    let mut chars = inline.chars();
    match chars.next() {
        Some('|' | '>') => {}
        _ => return false,
    }
    let rest: Vec<char> = chars.collect();
    match rest.first() {
        Some('+') => true,
        Some(digit) if digit.is_ascii_digit() => rest.get(1) == Some(&'+'),
        _ => false,
    }
}

fn settle(
    pending: &mut Option<Pending>,
    properties: &mut Vec<Property>,
    blanks: usize,
    source: &str,
) {
    let Some(entry) = pending.take() else {
        return;
    };
    let mut rest = entry.rest;
    if is_chomp_indented(&entry.inline) {
        for _ in 0..blanks {
            rest.push(String::new());
        }
    }
    let span = Span::new(entry.from, entry.to);
    let (value, raw) = match yaml::value_of(&entry.inline, &rest) {
        Some(value) => (value, None),
        None => (PropertyValue::Null, Some(span.slice(source).to_owned())),
    };
    properties.push(Property {
        key: entry.key,
        value,
        raw,
        span,
    });
}

pub(crate) fn parse(source: &str, lines: &[Line]) -> Option<(Frontmatter, usize)> {
    let first = lines.first()?;
    if first.text(source).trim_end() != "---" {
        return None;
    }
    let close_index = lines.iter().enumerate().skip(1).find_map(|(index, line)| {
        let trimmed = line.text(source).trim_end();
        (trimmed == "---" || trimmed == "...").then_some(index)
    })?;
    let open = Span::new(first.start, first.start + 3);
    let limit = close_index;
    let mut properties = Vec::new();
    let mut pending: Option<Pending> = None;
    let mut blanks = 0_usize;
    for line in lines.iter().take(limit).skip(1) {
        let text = line.text(source);
        if pending.is_some() && text.trim().is_empty() {
            blanks += 1;
            continue;
        }
        if pending.is_some() && yaml::is_continuation(text) {
            if let Some(entry) = pending.as_mut() {
                for _ in 0..blanks {
                    entry.rest.push(String::new());
                }
                entry.rest.push(text.to_owned());
                entry.to = line.end;
            }
            blanks = 0;
            continue;
        }
        settle(&mut pending, &mut properties, blanks, source);
        blanks = 0;
        if text.trim().is_empty() || text.starts_with('#') {
            continue;
        }
        let Some(parts) = key_line(text) else {
            let line = lines.get(close_index)?;
            return Some((
                Frontmatter {
                    span: Span::new(0, line.end),
                    open,
                    close: line.span(),
                    properties: Vec::new(),
                    unsupported: true,
                },
                close_index + 1,
            ));
        };
        let key = match yaml::scalar(parts.key) {
            Some(PropertyValue::Null) | None => parts.key.to_owned(),
            Some(value) => value.to_plain_string(),
        };
        let inline = if parts.value.starts_with('#') {
            String::new()
        } else {
            parts.value.to_owned()
        };
        pending = Some(Pending {
            key,
            inline,
            from: line.start,
            to: line.end,
            rest: Vec::new(),
        });
    }
    settle(&mut pending, &mut properties, blanks, source);
    let close_line = lines.get(close_index)?;
    Some((
        Frontmatter {
            span: Span::new(0, close_line.end),
            open,
            close: close_line.span(),
            properties,
            unsupported: false,
        },
        close_index + 1,
    ))
}
