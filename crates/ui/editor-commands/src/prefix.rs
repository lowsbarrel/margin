use crate::text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Marker<'text> {
    pub indent: &'text str,
    pub token: &'text str,
    pub number: Option<u64>,
    pub space: &'text str,
    pub task: &'text str,
}

impl Marker<'_> {
    pub(crate) fn len(&self) -> usize {
        self.indent.len() + self.token.len() + self.space.len() + self.task.len()
    }

    pub(crate) fn is_ordered(&self) -> bool {
        self.number.is_some()
    }

    pub(crate) fn delimiter(&self) -> &str {
        if self.is_ordered() {
            self.token.get(self.token.len() - 1..).unwrap_or_default()
        } else {
            ""
        }
    }

    pub(crate) fn digits(&self) -> &str {
        self.token
            .get(..self.token.len() - self.delimiter().len())
            .unwrap_or_default()
    }
}

fn spaces(text: &str) -> &str {
    text::leading_space(text)
}

fn token(text: &str) -> Option<(&str, Option<u64>)> {
    if text.starts_with(['-', '*', '+']) {
        return Some((text.get(..1)?, None));
    }
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 || digits > 9 || !text.get(digits..)?.starts_with(['.', ')']) {
        return None;
    }
    Some((text.get(..=digits)?, text.get(..digits)?.parse().ok()))
}

pub(crate) fn task_box(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('[')?;
    if !inner.starts_with([' ', 'x', 'X']) || inner.get(1..2) != Some("]") {
        return None;
    }
    let after = spaces(inner.get(2..)?);
    (!after.is_empty()).then(|| text.get(..3 + after.len()))?
}

pub(crate) fn marker(text: &str) -> Option<Marker<'_>> {
    let indent = spaces(text);
    let rest = text.get(indent.len()..)?;
    let (token, number) = token(rest)?;
    let rest = rest.get(token.len()..)?;
    let space = spaces(rest);
    let task = if space.is_empty() {
        ""
    } else {
        task_box(rest.get(space.len()..)?).unwrap_or_default()
    };
    Some(Marker {
        indent,
        token,
        number,
        space,
        task,
    })
}

pub(crate) fn spaced_marker(text: &str) -> Option<Marker<'_>> {
    marker(text).filter(|found| !found.space.is_empty())
}

pub(crate) fn heading(text: &str) -> Option<(usize, usize)> {
    let hashes = text.len() - text.trim_start_matches('#').len();
    let gap = spaces(text.get(hashes..)?).len();
    ((1..=6).contains(&hashes) && gap > 0).then_some((hashes, hashes + gap))
}

pub(crate) fn quote(text: &str) -> Option<usize> {
    let indent = spaces(text).len();
    let rest = text.get(indent..)?.strip_prefix('>')?;
    Some(indent + 1 + usize::from(rest.starts_with([' ', '\t'])))
}

pub(crate) fn quotes(text: &str) -> usize {
    let mut at = 0;
    while let Some(rest) = text.get(at..).and_then(|rest| rest.strip_prefix('>')) {
        at += 1 + usize::from(rest.starts_with([' ', '\t']));
    }
    at
}

pub(crate) fn is_empty_quote(text: &str) -> bool {
    let indent = spaces(text).len();
    let body = quotes(text.get(indent..).unwrap_or_default());
    body > 0 && indent + body == text.len()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LinePrefix<'text> {
    pub head: &'text str,
    pub marker: &'text str,
    pub gap: &'text str,
    pub task: &'text str,
}

impl LinePrefix<'_> {
    pub(crate) fn len(&self) -> usize {
        self.head.len() + self.marker.len() + self.gap.len() + self.task.len()
    }
}

pub(crate) fn line_prefix(text: &str) -> LinePrefix<'_> {
    let indent = spaces(text).len();
    let quoted = indent + quotes(text.get(indent..).unwrap_or_default());
    let inner = quoted + spaces(text.get(quoted..).unwrap_or_default()).len();
    let head = text.get(..inner).unwrap_or_default();
    let rest = text.get(inner..).unwrap_or_default();
    let found = token(rest).and_then(|(token, _)| {
        let after = rest.get(token.len()..)?;
        let gap = spaces(after);
        if gap.is_empty() {
            return None;
        }
        let task = task_box(after.get(gap.len()..)?).unwrap_or_default();
        Some((token, gap, task))
    });
    let (marker, gap, task) = found.unwrap_or_default();
    LinePrefix {
        head,
        marker,
        gap,
        task,
    }
}

pub(crate) fn block_prefix(text: &str) -> usize {
    let mut at = 0;
    loop {
        let rest = text.get(at..).unwrap_or_default();
        let step = heading(rest)
            .map(|(_, length)| length)
            .or_else(|| {
                let (token, _) = token(rest)?;
                let gap = spaces(rest.get(token.len()..)?);
                if gap.is_empty() {
                    return None;
                }
                let after = rest.get(token.len() + gap.len()..)?;
                let task = if token.len() == 1 {
                    task_box(after).map_or(0, str::len)
                } else {
                    0
                };
                Some(token.len() + gap.len() + task)
            })
            .or_else(|| {
                let after = rest.strip_prefix('>')?;
                Some(1 + usize::from(after.starts_with([' ', '\t'])))
            });
        match step {
            Some(step) if step > 0 => at += step,
            _ => return at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_list_markers() {
        let found = marker("  12) [x] done").expect("an ordered task item");
        assert_eq!(found.indent, "  ");
        assert_eq!(found.digits(), "12");
        assert_eq!(found.delimiter(), ")");
        assert_eq!(found.task, "[x] ");
        assert_eq!(found.len(), 10);
        assert!(spaced_marker("-x").is_none());
        assert!(marker("1234567890. x").is_none());
    }

    #[test]
    fn reads_line_prefixes() {
        let prefix = line_prefix("> > - [ ] task");
        assert_eq!(prefix.head, "> > ");
        assert_eq!(prefix.marker, "-");
        assert_eq!(prefix.len(), 10);
        assert_eq!(block_prefix("## > - [ ] x"), 11);
        assert!(is_empty_quote("  > >"));
        assert!(!is_empty_quote("> a"));
        assert_eq!(heading("### Title"), Some((3, 4)));
        assert_eq!(heading("#Title"), None);
    }
}
