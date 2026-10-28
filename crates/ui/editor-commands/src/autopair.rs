use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::mapping::{self, RangeEdit};
use crate::text::{self, Line};
use crate::tree;

const WRAPS: [char; 7] = ['*', '_', '`', '=', '~', '$', '%'];
const PAIRS: [char; 3] = ['*', '_', '`'];
const BRACKETS: [(char, char); 5] = [('(', ')'), ('[', ']'), ('{', '}'), ('\'', '\''), ('"', '"')];
const CLOSE_BEFORE: &str = ")]}:;>";

fn across_lines(open: char) -> Option<char> {
    match open {
        '*' | '_' | '"' | '\'' => Some(open),
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

struct Around {
    lead: usize,
    trail: usize,
    previous: Option<char>,
    next: Option<char>,
}

fn around(document: &Document, pos: usize, mark: char) -> Around {
    let line = Line::at(document, pos);
    let before = text::slice(document.text(), line.start, pos);
    let after = text::slice(document.text(), pos, line.end);
    let lead = before.chars().rev().take_while(|c| *c == mark).count();
    let trail = after.chars().take_while(|c| *c == mark).count();
    Around {
        lead,
        trail,
        previous: before.chars().rev().nth(lead),
        next: after.chars().nth(trail),
    }
}

fn boundary(character: Option<char>) -> bool {
    character.is_none_or(char::is_whitespace)
}

fn empty_pair(at: &Around) -> bool {
    at.lead == at.trail && boundary(at.previous) && boundary(at.next)
}

fn in_code_body(document: &Document, pos: usize) -> bool {
    tree::ancestors(document, pos)
        .iter()
        .rev()
        .find(|block| block.is_code_block())
        .is_some_and(|code| Line::at(document, code.span.start).end < pos)
}

fn wrap(range: Selection, open: &str, close: &str) -> RangeEdit {
    RangeEdit::new(
        vec![
            TextEdit::new(Span::empty(range.start()), open),
            TextEdit::new(Span::empty(range.end()), close),
        ],
        Selection::new(range.anchor + open.len(), range.head + open.len()),
    )
}

fn wrap_range(document: &Document, range: Selection, mark: char) -> RangeEdit {
    let source = document.text();
    let (from, to) = (range.start(), range.end());
    let fence = mark == '`'
        && from >= 2
        && text::slice(source, from - 2, from) == "``"
        && text::slice(source, to, to + 2) == "``";
    if !fence {
        let mark = mark.to_string();
        return wrap(range, &mark, &mark);
    }
    let (outer_from, outer_to) = (from - 2, to + 2);
    let open = if outer_from == Line::at(document, outer_from).start {
        "```\n"
    } else {
        "\n```\n"
    };
    let close = if outer_to == Line::at(document, outer_to).end {
        "\n```"
    } else {
        "\n```\n"
    };
    let body = text::slice(source, from, to);
    let start = outer_from + open.len();
    RangeEdit::new(
        vec![TextEdit::new(
            Span::new(outer_from, outer_to),
            format!("{open}{body}{close}"),
        )],
        Selection::new(start, start + body.len()),
    )
}

fn pair_range(document: &Document, range: Selection, mark: char) -> Option<RangeEdit> {
    let pos = range.head;
    let at = around(document, pos, mark);
    if empty_pair(&at) {
        let line_start = Line::at(document, pos).start;
        let indent = text::slice(
            document.text(),
            line_start,
            pos.saturating_sub(2).max(line_start),
        );
        if mark == '`' && at.lead == 2 && indent.trim().is_empty() {
            return Some(RangeEdit::replace(
                Span::new(pos - 2, pos + 2),
                format!("```\n{indent}```"),
                pos + 1,
            ));
        }
        return Some(RangeEdit::replace(
            Span::empty(pos),
            format!("{mark}{mark}"),
            pos + 1,
        ));
    }
    (at.trail > 0 && !at.next.is_some_and(text::is_word_char))
        .then(|| RangeEdit::new(Vec::new(), Selection::cursor(pos + mark.len_utf8())))
}

fn type_mark(
    document: &Document,
    selections: &[Selection],
    head: usize,
    mark: char,
) -> Option<Change> {
    if in_code_body(document, head) {
        return None;
    }
    let wrapping = selections.iter().any(|range| !range.is_empty());
    if !wrapping && !PAIRS.contains(&mark) {
        return None;
    }
    let mut handled = false;
    let change = mapping::by_range(selections, |range| {
        let spec = match (wrapping, range.is_empty()) {
            (true, true) => None,
            (true, false) => Some(wrap_range(document, range, mark)),
            (false, _) => pair_range(document, range, mark),
        };
        handled |= spec.is_some();
        Some(spec.unwrap_or_else(|| {
            RangeEdit::replace(
                range.span(),
                mark.to_string(),
                range.start() + mark.len_utf8(),
            )
        }))
    })?;
    handled.then_some(change)
}

fn spans_lines(document: &Document, range: Selection) -> bool {
    document.line_of(range.start()) != document.line_of(range.end())
}

fn pair_across_lines(document: &Document, selections: &[Selection], open: char) -> Option<Change> {
    let close = across_lines(open)?;
    if !selections.iter().any(|range| spans_lines(document, *range)) {
        return None;
    }
    let (open, close) = (open.to_string(), close.to_string());
    mapping::by_range(selections, |range| {
        Some(if spans_lines(document, range) {
            RangeEdit::replace(
                range.span(),
                format!("{open}{close}"),
                range.start() + open.len(),
            )
        } else {
            wrap(range, &open, &close)
        })
    })
}

fn single_empty_pair(document: &Document, selections: &[Selection]) -> Option<(char, usize)> {
    let [range] = selections else {
        return None;
    };
    let source = document.text();
    let mark = text::char_before(source, range.head).filter(|mark| PAIRS.contains(mark))?;
    if !range.is_empty() || text::char_at(source, range.head) != Some(mark) {
        return None;
    }
    empty_pair(&around(document, range.head, mark)).then_some((mark, range.head))
}

fn close_brackets(document: &Document, selections: &[Selection], typed: char) -> Option<Change> {
    let source = document.text();
    let (open, close) = BRACKETS
        .iter()
        .copied()
        .find(|(open, close)| *open == typed || *close == typed)?;
    let (open_text, close_text) = (open.to_string(), close.to_string());
    if typed == close && typed != open {
        return mapping::by_range(selections, |range| {
            (range.is_empty() && text::char_at(source, range.head) == Some(close)).then(|| {
                RangeEdit::new(Vec::new(), Selection::cursor(range.head + close.len_utf8()))
            })
        });
    }
    mapping::by_range(selections, |range| {
        if !range.is_empty() {
            return Some(wrap(range, &open_text, &close_text));
        }
        let next = text::char_at(source, range.head);
        let pair = || {
            RangeEdit::replace(
                Span::empty(range.head),
                format!("{open}{close}"),
                range.head + open.len_utf8(),
            )
        };
        if open != close {
            return next
                .is_none_or(|next| next.is_whitespace() || CLOSE_BEFORE.contains(next))
                .then(pair);
        }
        if next == Some(close) {
            return Some(RangeEdit::new(
                Vec::new(),
                Selection::cursor(range.head + close.len_utf8()),
            ));
        }
        let word_after = next.is_some_and(text::is_word_char);
        let word_before = text::char_before(source, range.head).is_some_and(text::is_word_char);
        (!word_after && !word_before).then(pair)
    })
}

#[must_use]
pub fn autopair_input(
    document: &Document,
    selections: &[Selection],
    primary: usize,
    text: &str,
) -> Option<Change> {
    let mut characters = text.chars();
    let (Some(typed), None) = (characters.next(), characters.next()) else {
        return None;
    };
    let main = *selections.get(primary).or_else(|| selections.last())?;
    if let Some(change) = pair_across_lines(document, selections, typed) {
        return Some(change);
    }
    match typed {
        '\'' if main.is_empty() && text::char_before(document.text(), main.head) == Some(']') => {
            mapping::by_range(selections, |range| {
                Some(RangeEdit::replace(range.span(), "'", range.start() + 1))
            })
        }
        ' ' => {
            let (mark, head) = single_empty_pair(document, selections)?;
            (around(document, head, mark).lead == 1).then(|| {
                Change::new(vec![TextEdit::new(
                    Span::new(head, head + mark.len_utf8()),
                    " ",
                )])
                .with_selections(vec![Selection::cursor(head + 1)])
            })
        }
        mark if WRAPS.contains(&mark) => type_mark(document, selections, main.head, mark),
        _ => close_brackets(document, selections, typed),
    }
}

#[must_use]
pub fn delete_pair_backward(document: &Document, selections: &[Selection]) -> Option<Change> {
    if let Some((mark, head)) = single_empty_pair(document, selections) {
        let width = mark.len_utf8();
        return Some(
            Change::new(vec![TextEdit::new(
                Span::new(head - width, head + width),
                "",
            )])
            .with_selections(vec![Selection::cursor(head - width)]),
        );
    }
    let source = document.text();
    mapping::by_range(selections, |range| {
        if !range.is_empty() {
            return None;
        }
        let before = text::char_before(source, range.head)?;
        let (open, close) = BRACKETS.iter().copied().find(|(open, _)| *open == before)?;
        (text::char_at(source, range.head) == Some(close)).then(|| {
            let from = range.head - open.len_utf8();
            RangeEdit::replace(Span::new(from, range.head + close.len_utf8()), "", from)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{primary, run};

    fn typed(source: &str, text: &str) -> Option<String> {
        run(source, |d, s| autopair_input(d, s, primary(s), text))
    }

    #[test]
    fn pairs_emphasis_marks_and_skips_over_them() {
        assert_eq!(typed("a |", "*").as_deref(), Some("a *|*"));
        assert_eq!(typed("a *|*", "*").as_deref(), Some("a **|**"));
        assert_eq!(typed("a *b|*", "*").as_deref(), Some("a *b*|"));
        assert_eq!(typed("wo|rd", "*"), None);
        assert_eq!(typed("a |", "="), None);
    }

    #[test]
    fn wraps_a_selection_in_a_mark() {
        assert_eq!(typed("a «b» c", "=").as_deref(), Some("a =«b»= c"));
        assert_eq!(
            typed("``«code»``", "`").as_deref(),
            Some("```\n«code»\n```")
        );
    }

    #[test]
    fn opens_a_code_fence_from_two_backticks() {
        assert_eq!(typed("``|``", "`").as_deref(), Some("```|\n```"));
    }

    #[test]
    fn unpairs_on_space_and_keeps_link_apostrophes_plain() {
        assert_eq!(typed("*|*", " ").as_deref(), Some("* |"));
        assert_eq!(typed("[[link]]|", "'").as_deref(), Some("[[link]]'|"));
    }

    #[test]
    fn closes_brackets_and_quotes() {
        assert_eq!(typed("f|", "(").as_deref(), Some("f(|)"));
        assert_eq!(typed("f |", "(").as_deref(), Some("f (|)"));
        assert_eq!(typed("(a|)", ")").as_deref(), Some("(a)|"));
        assert_eq!(typed("say |", "\"").as_deref(), Some("say \"|\""));
        assert_eq!(typed("don|", "'"), None);
        assert_eq!(typed("«a\nb»", "(").as_deref(), Some("(|)"));
    }

    #[test]
    fn leaves_code_blocks_alone() {
        assert_eq!(typed("```\nx |\n```", "*"), None);
    }

    #[test]
    fn deletes_empty_pairs() {
        let delete = |source| run(source, delete_pair_backward);
        assert_eq!(delete("a *|*").as_deref(), Some("a |"));
        assert_eq!(delete("(|)").as_deref(), Some("|"));
        assert_eq!(delete("(a|)"), None);
    }
}
