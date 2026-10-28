use crate::frontmatter::KeyLine;

const KEY_EXCLUDED: &[char] = &[
    ' ', '\t', '\n', '#', ',', '[', ']', '{', '}', '&', '*', '!', '|', '>', '\'', '"', '%', '@',
    '`',
];

fn quoted_key_end(text: &str) -> Option<usize> {
    let quote = text.chars().next()?;
    let mut chars = text.char_indices();
    chars.next();
    let mut escaped = false;
    for (at, ch) in chars {
        if quote == '"' && escaped {
            escaped = false;
            continue;
        }
        if quote == '"' && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch != quote {
            continue;
        }
        if quote == '\''
            && text
                .get(at + 1..)
                .is_some_and(|rest| rest.starts_with('\''))
        {
            continue;
        }
        return Some(at + 1);
    }
    None
}

fn key_end(text: &str) -> Option<usize> {
    let first = text.chars().next()?;
    if first == '"' || first == '\'' {
        let end = quoted_key_end(text)?;
        return text
            .get(end..)
            .is_some_and(|rest| rest.starts_with(':'))
            .then_some(end);
    }
    if matches!(first, '-' | '?' | ':') {
        let second = text.chars().nth(1);
        if second.is_none_or(char::is_whitespace) {
            return None;
        }
    } else if KEY_EXCLUDED.contains(&first) {
        return None;
    }
    for (at, byte) in text.as_bytes().iter().enumerate() {
        if *byte != b':' {
            continue;
        }
        let rest = text.get(at + 1..).unwrap_or_default();
        if rest.is_empty() || rest.starts_with([' ', '\t']) {
            return Some(at);
        }
    }
    None
}

#[must_use]
pub(crate) fn key_line(text: &str) -> Option<KeyLine<'_>> {
    let end = key_end(text)?;
    let key = text.get(..end)?;
    let value = text.get(end + 1..).unwrap_or_default().trim();
    Some(KeyLine { key, value })
}
