fn scheme_len(text: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let mut length = 1;
    for (at, ch) in chars {
        if ch == ':' {
            return (2..=32).contains(&length).then_some(at + 1);
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.' | '-') {
            length += 1;
            continue;
        }
        return None;
    }
    None
}

#[must_use]
pub(crate) fn autolink_target(inner: &str) -> Option<(String, bool)> {
    if inner.is_empty() || inner.contains(char::is_whitespace) || inner.contains('<') {
        return None;
    }
    if scheme_len(inner).is_some() {
        return Some((inner.to_owned(), false));
    }
    let (local, domain) = inner.split_once('@')?;
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return None;
    }
    Some((format!("mailto:{inner}"), true))
}

fn is_local(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-')
}

fn email_at(rest: &str) -> Option<(usize, String)> {
    let bytes = rest.as_bytes();
    let mut end = 0;
    while bytes.get(end).copied().is_some_and(is_local) {
        end += 1;
    }
    if end == 0 || bytes.get(end) != Some(&b'@') {
        return None;
    }
    let local = rest.get(..end)?;
    if local.starts_with('.') || local.ends_with('.') {
        return None;
    }
    let mut scan = end + 1;
    let mut last_dot = None;
    while let Some(byte) = bytes.get(scan).copied() {
        if byte.is_ascii_alphanumeric() || byte == b'-' {
            scan += 1;
        } else if byte == b'.' {
            last_dot = Some(scan);
            scan += 1;
        } else {
            break;
        }
    }
    let domain = rest.get(end + 1..scan)?;
    let dot = last_dot?;
    let tld = rest.get(dot + 1..scan)?;
    if tld.len() < 2 || !tld.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return None;
    }
    if domain.ends_with('.') {
        return None;
    }
    Some((scan, format!("mailto:{local}@{domain}")))
}

fn trim_url_end(rest: &str, mut end: usize) -> usize {
    loop {
        let Some(slice) = rest.get(..end) else {
            return end;
        };
        let Some(last) = slice.chars().next_back() else {
            return end;
        };
        if matches!(
            last,
            '.' | ',' | ':' | ';' | '!' | '?' | '"' | '\'' | '*' | '_' | '~'
        ) {
            end -= last.len_utf8();
            continue;
        }
        if last == ')' {
            let opens = slice.matches('(').count();
            let closes = slice.matches(')').count();
            if closes > opens {
                end -= 1;
                continue;
            }
        }
        return end;
    }
}

#[must_use]
pub(crate) fn bare_link(rest: &str) -> Option<(String, bool, usize)> {
    for prefix in ["https://", "http://", "www."] {
        let Some(tail) = rest.strip_prefix(prefix) else {
            continue;
        };
        if tail.is_empty() || tail.starts_with(['.', '-']) {
            continue;
        }
        let mut end = 0;
        for (offset, ch) in rest.char_indices() {
            if ch.is_whitespace() || ch == '<' {
                break;
            }
            end = offset + ch.len_utf8();
        }
        let end = trim_url_end(rest, end);
        if end <= prefix.len() {
            continue;
        }
        let body = rest.get(..end)?;
        let destination = if prefix == "www." {
            format!("http://{body}")
        } else {
            body.to_owned()
        };
        return Some((destination, false, end));
    }
    let (end, address) = email_at(rest)?;
    Some((address, true, end))
}
