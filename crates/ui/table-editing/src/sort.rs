use std::cmp::Ordering;

fn wiki(rest: &str) -> Option<(String, usize)> {
    let open = usize::from(rest.starts_with('!'));
    let body = rest.get(open..)?.strip_prefix("[[")?;
    let target_end = body.find([']', '|']).unwrap_or(body.len());
    let after = body.get(target_end..)?;
    let (label, tail) = match after.strip_prefix('|') {
        Some(alias) => {
            let end = alias.find(']').unwrap_or(alias.len());
            (alias.get(..end)?, alias.get(end..)?)
        }
        None => (body.get(..target_end)?, after),
    };
    tail.starts_with("]]").then(|| {
        let used = rest.len() - tail.len() + 2;
        (label.to_owned(), used)
    })
}

fn link(rest: &str) -> Option<(String, usize)> {
    let open = usize::from(rest.starts_with('!'));
    let body = rest.get(open..)?.strip_prefix('[')?;
    let close = body.find(']')?;
    let label = body.get(..close)?;
    let url = body.get(close + 1..)?.strip_prefix('(')?;
    let end = url.find(')')?;
    Some((label.to_owned(), open + 1 + close + 2 + end + 1))
}

fn sort_key(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(rest) = text.get(at..) {
        let Some(ch) = rest.chars().next() else {
            break;
        };
        if matches!(ch, '!' | '[')
            && let Some((label, used)) = wiki(rest).or_else(|| link(rest))
        {
            out.push_str(&label);
            at += used;
            continue;
        }
        out.push(ch);
        at += ch.len_utf8();
    }
    out.retain(|ch| !matches!(ch, '*' | '_' | '~' | '=' | '`' | '\\'));
    out.trim().to_owned()
}

fn chunks(text: &str) -> Vec<(bool, String)> {
    let mut parts: Vec<(bool, String)> = Vec::new();
    for ch in text.chars() {
        let digit = ch.is_ascii_digit();
        match parts.last_mut() {
            Some((kind, part)) if *kind == digit => part.extend(ch.to_lowercase()),
            _ => parts.push((digit, ch.to_lowercase().collect())),
        }
    }
    parts
}

fn compare_chunk(a: &(bool, String), b: &(bool, String)) -> Ordering {
    match (a, b) {
        ((true, x), (true, y)) => {
            let x = x.trim_start_matches('0');
            let y = y.trim_start_matches('0');
            x.len().cmp(&y.len()).then_with(|| x.cmp(y))
        }
        ((true, _), (false, _)) => Ordering::Less,
        ((false, _), (true, _)) => Ordering::Greater,
        ((false, x), (false, y)) => x.cmp(y),
    }
}

pub(crate) fn compare_cells(a: &str, b: &str) -> Ordering {
    let left = chunks(&sort_key(a));
    let right = chunks(&sort_key(b));
    for (x, y) in left.iter().zip(&right) {
        let order = compare_chunk(x, y);
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markup_from_keys() {
        assert_eq!(sort_key("**Bold** [[Note\\|Alias]]"), "Bold Alias");
        assert_eq!(
            sort_key("[[Target]] [label](url) ![img](a.png)"),
            "Target label img"
        );
        assert_eq!(sort_key(" `code` "), "code");
    }

    #[test]
    fn compares_numbers_naturally_and_ignores_case() {
        assert_eq!(compare_cells("item 10", "item 9"), Ordering::Greater);
        assert_eq!(compare_cells("apple", "Banana"), Ordering::Less);
        assert_eq!(compare_cells("A", "a"), Ordering::Equal);
    }
}
