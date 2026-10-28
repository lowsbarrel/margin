use margin_markdown::Span;
use margin_text_buffer::Selection;

#[must_use]
pub fn nearest_match(matches: &[Span], selection: Selection) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let selected = selection.span();
    let at = matches.partition_point(|span| span.start < selected.start);
    if matches.get(at) == Some(&selected) {
        return Some(at);
    }
    let after = matches.partition_point(|span| span.start < selection.head);
    Some(if after < matches.len() { after } else { 0 })
}

#[must_use]
pub fn next_match(matches: &[Span], selection: Selection) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let after = matches.partition_point(|span| span.start < selection.end());
    Some(if after < matches.len() { after } else { 0 })
}

#[must_use]
pub fn previous_match(matches: &[Span], selection: Selection) -> Option<usize> {
    let before = matches.partition_point(|span| span.end <= selection.start());
    if before > 0 {
        return Some(before - 1);
    }
    matches
        .last()
        .filter(|span| span.start >= selection.end())
        .map(|_| matches.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches() -> Vec<Span> {
        vec![Span::new(2, 4), Span::new(10, 12), Span::new(20, 22)]
    }

    #[test]
    fn nearest_is_the_selected_match() {
        assert_eq!(nearest_match(&matches(), Selection::new(10, 12)), Some(1));
        assert_eq!(nearest_match(&matches(), Selection::new(12, 10)), Some(1));
    }

    #[test]
    fn nearest_is_the_first_after_the_head() {
        assert_eq!(nearest_match(&matches(), Selection::cursor(5)), Some(1));
        assert_eq!(nearest_match(&matches(), Selection::cursor(30)), Some(0));
        assert_eq!(nearest_match(&[], Selection::cursor(0)), None);
    }

    #[test]
    fn next_wraps_to_the_first() {
        assert_eq!(next_match(&matches(), Selection::new(2, 4)), Some(1));
        assert_eq!(next_match(&matches(), Selection::new(20, 22)), Some(0));
        assert_eq!(
            next_match(&[Span::new(1, 2)], Selection::new(1, 2)),
            Some(0)
        );
    }

    #[test]
    fn previous_wraps_to_the_last() {
        assert_eq!(previous_match(&matches(), Selection::new(10, 12)), Some(0));
        assert_eq!(previous_match(&matches(), Selection::new(2, 4)), Some(2));
        assert_eq!(previous_match(&matches(), Selection::cursor(0)), Some(2));
    }

    #[test]
    fn previous_of_the_only_selected_match_is_none() {
        assert_eq!(
            previous_match(&[Span::new(1, 2)], Selection::new(1, 2)),
            None
        );
        assert_eq!(previous_match(&[], Selection::cursor(0)), None);
    }
}
