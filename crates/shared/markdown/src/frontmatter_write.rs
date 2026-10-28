use crate::frontmatter::PropertyValue;
use crate::frontmatter_key::key_line;
use crate::yaml_scalar::{bool_value, is_null, parse_number};

const UNQUOTED_UNSAFE_START: &[char] = &[
    '\n', '\t', ' ', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@',
    '`',
];

pub(crate) fn is_plain_unsafe(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if chars
        .first()
        .is_some_and(|first| UNQUOTED_UNSAFE_START.contains(first))
    {
        return true;
    }
    if chars.len() == 1 && matches!(chars.first(), Some('?' | '-')) {
        return true;
    }
    if matches!(chars.first(), Some('?' | '-')) && matches!(chars.get(1), Some('\t' | ' ')) {
        return true;
    }
    for pair in chars.windows(2) {
        let (first, second) = (pair.first().copied(), pair.get(1).copied());
        if matches!(first, Some('\n' | ':')) && matches!(second, Some('\t' | ' ')) {
            return true;
        }
        if matches!(first, Some(' ' | '\t')) && second == Some('\n') {
            return true;
        }
        if matches!(first, Some('\n' | '\t' | ' ')) && second == Some('#') {
            return true;
        }
    }
    matches!(chars.last(), Some('\n' | '\t' | ' ' | ':'))
}

#[must_use]
pub(crate) fn format_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_owned()
        } else {
            "Infinity".to_owned()
        };
    }
    value.to_string()
}

fn json_quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            control if (control as u32) < 0x20 => {
                out.push_str("\\u");
                let value = control as u32;
                for shift in [12, 8, 4, 0] {
                    let digit = (value >> shift) & 0xf;
                    out.push(char::from_digit(digit, 16).unwrap_or('0'));
                }
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn quoted(text: &str) -> String {
    if text.contains('"') && !text.contains('\'') {
        let mut out = String::with_capacity(text.len() + 2);
        out.push('\'');
        for ch in text.chars() {
            if ch == '\'' {
                out.push('\'');
            }
            out.push(ch);
        }
        out.push('\'');
        return out;
    }
    json_quote(text)
}

fn yaml_string(text: &str, key: bool) -> String {
    if key && text.contains('\n') {
        return quoted(text);
    }
    if !text.is_empty()
        && !is_plain_unsafe(text)
        && !is_null(text)
        && bool_value(text).is_none()
        && parse_number(text).is_none()
    {
        return text.to_owned();
    }
    quoted(text)
}

fn block_text(text: &str, indent: &str) -> String {
    let trailing = text.len() - text.trim_end_matches('\n').len();
    let chomp = match trailing {
        0 => "-",
        1 => "",
        _ => "+",
    };
    let body = text.get(..text.len() - trailing.min(1)).unwrap_or_default();
    let mut out = String::new();
    out.push('|');
    if body.starts_with([' ', '\t']) {
        out.push('2');
    }
    out.push_str(chomp);
    for line in body.split('\n') {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(indent);
            out.push_str("  ");
            out.push_str(line);
        }
    }
    out
}

fn list_item(item: &str) -> String {
    let mut out = String::from("  - ");
    if item.contains('\n') {
        out.push_str(&block_text(item, "  "));
    } else {
        out.push_str(&yaml_string(item, false));
    }
    out
}

#[must_use]
pub fn serialize_property(key: &str, value: &PropertyValue) -> String {
    let mut out = yaml_string(key, true);
    match value {
        PropertyValue::Null => out.push(':'),
        PropertyValue::List(items) if items.is_empty() => out.push(':'),
        PropertyValue::Text(text) if text.is_empty() => out.push(':'),
        PropertyValue::List(items) => {
            out.push(':');
            for item in items {
                out.push('\n');
                out.push_str(&list_item(item));
            }
        }
        PropertyValue::Text(text) if text.contains('\n') => {
            out.push_str(": ");
            out.push_str(&block_text(text, ""));
        }
        PropertyValue::Text(text) => {
            out.push_str(": ");
            out.push_str(&yaml_string(text, false));
        }
        PropertyValue::Bool(value) => {
            out.push_str(": ");
            out.push_str(if *value { "true" } else { "false" });
        }
        PropertyValue::Number(value) => {
            out.push_str(": ");
            out.push_str(&format_number(*value));
        }
    }
    out
}

#[must_use]
pub fn rename_source(raw: &str, key: &str) -> String {
    let first = raw.split('\n').next().unwrap_or_default();
    match key_line(first) {
        Some(parts) => {
            let mut out = serialize_property(key, &PropertyValue::Null);
            out.push_str(raw.get(parts.key.len() + 1..).unwrap_or_default());
            out
        }
        None => raw.to_owned(),
    }
}
