use crate::frontmatter::PropertyValue;
use crate::yaml_scalar::plain_scalar;

fn strip_comment(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        if bytes.get(at) == Some(&b'#') {
            let mut back = at;
            while back > 0 && matches!(bytes.get(back - 1), Some(b' ' | b'\t')) {
                back -= 1;
            }
            if back < at {
                return text.get(..back).unwrap_or_default();
            }
        }
        at += 1;
    }
    text
}

fn json_string(text: &str) -> Option<String> {
    let inner = text.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = inner.char_indices();
    while let Some((at, ch)) = chars.next() {
        match ch {
            '"' => {
                return (at + 1 == inner.len()).then_some(out);
            }
            '\\' => {
                let (_, escape) = chars.next()?;
                match escape {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'u' => {
                        let mut value = 0_u32;
                        for _ in 0..4 {
                            let (_, digit) = chars.next()?;
                            value = value * 16 + digit.to_digit(16)?;
                        }
                        out.push(char::from_u32(value).unwrap_or('\u{fffd}'));
                    }
                    _ => return None,
                }
            }
            control if (control as u32) < 0x20 => return None,
            other => out.push(other),
        }
    }
    None
}

fn single_quoted(text: &str) -> Option<String> {
    let inner = text.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut out = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            if chars.peek() != Some(&'\'') {
                return None;
            }
            chars.next();
        }
        out.push(ch);
    }
    Some(out)
}

#[must_use]
pub(crate) fn scalar(source: &str) -> Option<PropertyValue> {
    let text = source.trim();
    if text.starts_with('"') {
        return json_string(text).map(PropertyValue::Text);
    }
    if text.starts_with('\'') {
        return single_quoted(text).map(PropertyValue::Text);
    }
    let plain = strip_comment(text);
    if plain.starts_with(['&', '*', '!', '{', '[', '|', '>', '@', '`', '%']) {
        return None;
    }
    Some(plain_scalar(plain))
}

#[must_use]
pub(crate) fn flow_items(text: &str) -> Option<Vec<String>> {
    if !text.starts_with('[') || !text.ends_with(']') || text.len() < 2 || text.contains('\n') {
        return None;
    }
    let inner = text.get(1..text.len() - 1)?;
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in inner.chars() {
        match quote {
            Some(open) => {
                current.push(ch);
                if ch == open {
                    quote = None;
                }
            }
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            None if ch == '[' || ch == ']' || ch == '{' || ch == '}' => return None,
            None if ch == ',' => {
                items.push(current.clone());
                current.clear();
            }
            None => current.push(ch),
        }
    }
    if quote.is_some() {
        return None;
    }
    items.push(current);
    let mut values = Vec::new();
    for item in items {
        if item.trim().is_empty() {
            continue;
        }
        let parsed = scalar(&item)?;
        if parsed == PropertyValue::Null {
            return None;
        }
        values.push(parsed.to_plain_string());
    }
    Some(values)
}

fn fold_paragraphs(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut folded = String::with_capacity(text.len());
    for (at, ch) in chars.iter().enumerate() {
        if *ch == '\n'
            && at > 0
            && chars.get(at - 1) != Some(&'\n')
            && chars.get(at + 1).is_some_and(|next| *next != '\n')
        {
            folded.push(' ');
        } else {
            folded.push(*ch);
        }
    }
    let mut out = String::with_capacity(folded.len());
    let mut previous_newline = false;
    for ch in folded.chars() {
        if ch == '\n' {
            if previous_newline {
                continue;
            }
            previous_newline = true;
        } else {
            previous_newline = false;
        }
        out.push(ch);
    }
    out
}

fn block_scalar_header(header: &str) -> Option<char> {
    let mut chars = header.chars();
    let style = chars.next()?;
    if style != '|' && style != '>' {
        return None;
    }
    let rest: Vec<char> = chars.collect();
    let mut at = 0;
    let mut chomp = ' ';
    if let Some(marker @ ('-' | '+')) = rest.get(at).copied() {
        chomp = marker;
        at += 1;
    }
    if rest.get(at).is_some_and(char::is_ascii_digit) {
        at += 1;
    }
    if matches!(rest.get(at), Some('-' | '+')) {
        at += 1;
    }
    if at != rest.len() {
        return None;
    }
    Some(chomp)
}

#[must_use]
pub(crate) fn is_block_scalar_header(text: &str) -> bool {
    block_scalar_header(text).is_some()
}

#[must_use]
pub(crate) fn block_scalar(header: &str, lines: &[String]) -> Option<String> {
    let chomp = block_scalar_header(header)?;
    let style = header.chars().next()?;
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches(' ').len())
        .min();
    let mut text = lines
        .iter()
        .map(|line| {
            let cut = indent.map_or(line.len(), |value| value.min(line.len()));
            line.get(cut..).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\n");
    if style == '>' {
        text = fold_paragraphs(&text);
    }
    if chomp != '+' {
        text = text.trim_end_matches('\n').to_owned();
    }
    if chomp == ' ' && !text.is_empty() {
        text.push('\n');
    }
    Some(text)
}

#[must_use]
pub(crate) fn is_continuation(text: &str) -> bool {
    if text.starts_with([' ', '\t']) {
        return true;
    }
    match text.strip_prefix('-') {
        Some(rest) => rest.is_empty() || rest.starts_with([' ', '\t']),
        None => false,
    }
}

enum ListItem {
    Missing,
    Empty,
    Value(String),
}

fn list_item_value(line: &str) -> ListItem {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let Some(rest) = trimmed.strip_prefix('-') else {
        return ListItem::Missing;
    };
    if rest.is_empty() {
        return ListItem::Empty;
    }
    if !rest.starts_with([' ', '\t']) {
        return ListItem::Missing;
    }
    ListItem::Value(rest.trim_matches([' ', '\t']).to_owned())
}

fn starts_unsafe_item(text: &str) -> bool {
    match text.chars().next() {
        Some('[' | '{') => true,
        Some('-' | '?') => {
            let rest = text.get(1..).unwrap_or_default();
            rest.is_empty() || rest.starts_with([' ', '\t'])
        }
        _ => false,
    }
}

fn has_colon_space(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte == b':' {
            match bytes.get(at + 1) {
                None => return true,
                Some(next) if next.is_ascii_whitespace() => return true,
                _ => {}
            }
        }
    }
    false
}

#[must_use]
pub(crate) fn list_value(lines: &[String]) -> Option<Vec<String>> {
    let mut values = Vec::new();
    for line in lines {
        let source = match list_item_value(line) {
            ListItem::Missing => return None,
            ListItem::Empty => continue,
            ListItem::Value(source) => source,
        };
        let parsed = scalar(&source)?;
        if starts_unsafe_item(&source) {
            return None;
        }
        if !source.starts_with(['\'', '"']) && has_colon_space(&source) {
            return None;
        }
        if parsed != PropertyValue::Null {
            values.push(parsed.to_plain_string());
        }
    }
    Some(values)
}

#[must_use]
pub(crate) fn value_of(inline: &str, rest: &[String]) -> Option<PropertyValue> {
    if inline.is_empty() {
        return if rest.is_empty() {
            Some(PropertyValue::Null)
        } else {
            list_value(rest).map(PropertyValue::List)
        };
    }
    if inline.starts_with('[') {
        return if rest.is_empty() {
            flow_items(inline).map(PropertyValue::List)
        } else {
            None
        };
    }
    if is_block_scalar_header(inline) {
        return block_scalar(inline, rest).map(PropertyValue::Text);
    }
    if !rest.is_empty() {
        if inline.starts_with(['\'', '"', '[', '{']) {
            return None;
        }
        let mut joined = inline.to_owned();
        for line in rest {
            joined.push(' ');
            joined.push_str(line.trim());
        }
        return Some(PropertyValue::Text(joined));
    }
    scalar(inline)
}
