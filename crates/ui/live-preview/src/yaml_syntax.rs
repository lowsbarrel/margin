use margin_markdown::Span;

use crate::line_builder::{LineBuilder, Mark};
use crate::line_view::SyntaxRole;

pub(crate) fn highlight(builder: &mut LineBuilder<'_>, fence: bool) {
    let line = builder.line();
    let text = line.slice(builder.source());
    if fence {
        builder.mark_line(Mark::Markup);
        return;
    }
    let indent = text.len() - text.trim_start().len();
    let body = text.get(indent..).unwrap_or_default();
    if body.starts_with('#') {
        builder.mark_line(Mark::Syntax(SyntaxRole::Comment));
        return;
    }
    if let Some(rest) = body.strip_prefix('-')
        && (rest.is_empty() || rest.starts_with([' ', '\t']))
    {
        let dash = line.start + indent;
        builder.mark(Span::new(dash, dash + 1), Mark::Markup);
        value(builder, dash + 1);
        return;
    }
    if indent == 0
        && let Some(colon) = key_end(body)
    {
        builder.mark(
            Span::new(line.start, line.start + colon),
            Mark::Syntax(SyntaxRole::Attribute),
        );
        builder.mark(
            Span::new(line.start + colon, line.start + colon + 1),
            Mark::Markup,
        );
        value(builder, line.start + colon + 1);
        return;
    }
    value(builder, line.start + indent);
}

fn key_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut quote = None;
    for (at, byte) in bytes.iter().enumerate() {
        match (quote, *byte) {
            (Some(open), current) if current == open => quote = None,
            (None, b'"' | b'\'') if at == 0 => quote = Some(*byte),
            (None, b':') => {
                let next = bytes.get(at + 1);
                if next.is_none_or(|next| matches!(next, b' ' | b'\t')) {
                    return (at > 0).then_some(at);
                }
            }
            _ => {}
        }
    }
    None
}

fn value(builder: &mut LineBuilder<'_>, from: usize) {
    let line = builder.line();
    let text = Span::new(from, line.end).slice(builder.source());
    let lead = text.len() - text.trim_start().len();
    let start = from + lead;
    let body = text.get(lead..).unwrap_or_default();
    if body.is_empty() {
        return;
    }
    let comment = if body.starts_with('#') {
        Some(0)
    } else if body.starts_with(['"', '\'']) {
        None
    } else {
        body.find(" #")
            .or_else(|| body.find("\t#"))
            .map(|at| at + 1)
    };
    if let Some(at) = comment {
        builder.mark(
            Span::new(start + at, line.end),
            Mark::Syntax(SyntaxRole::Comment),
        );
    }
    let scalar = body
        .get(..comment.unwrap_or(body.len()))
        .unwrap_or_default();
    let scalar_len = scalar.trim_end().len();
    if scalar_len == 0 {
        return;
    }
    let role = role_of(scalar.trim_end());
    builder.mark(Span::new(start, start + scalar_len), Mark::Syntax(role));
}

fn role_of(scalar: &str) -> SyntaxRole {
    if matches!(
        scalar,
        "true" | "True" | "TRUE" | "false" | "False" | "FALSE" | "null" | "Null" | "NULL" | "~"
    ) || scalar.starts_with(['|', '>'])
    {
        return SyntaxRole::Keyword;
    }
    let unsigned = scalar.trim_start_matches(['-', '+']);
    let numeric = !unsigned.is_empty()
        && (unsigned.parse::<f64>().is_ok_and(f64::is_finite)
            || unsigned.starts_with("0x")
            || unsigned.starts_with("0o")
            || matches!(
                unsigned,
                ".inf" | ".Inf" | ".INF" | ".nan" | ".NaN" | ".NAN"
            ));
    if numeric {
        SyntaxRole::Number
    } else {
        SyntaxRole::String
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_view::LineDecor;

    fn roles(text: &str) -> Vec<(String, Option<SyntaxRole>, bool)> {
        let mut builder = LineBuilder::new(text, Span::new(0, text.len()));
        highlight(&mut builder, false);
        let view = builder.finish(LineDecor::default());
        view.runs
            .iter()
            .map(|run| {
                (
                    view.text
                        .get(run.range.clone())
                        .unwrap_or_default()
                        .to_owned(),
                    run.style.syntax,
                    run.style.markup,
                )
            })
            .collect()
    }

    #[test]
    fn keys_values_and_comments_get_roles() {
        assert_eq!(
            roles("count: 3 # n"),
            vec![
                ("count".to_owned(), Some(SyntaxRole::Attribute), false),
                (":".to_owned(), None, true),
                ("3".to_owned(), Some(SyntaxRole::Number), false),
                ("# n".to_owned(), Some(SyntaxRole::Comment), false),
            ]
        );
        assert_eq!(
            roles("  - done"),
            vec![
                ("-".to_owned(), None, true),
                ("done".to_owned(), Some(SyntaxRole::String), false),
            ]
        );
        assert_eq!(
            roles("draft: true"),
            vec![
                ("draft".to_owned(), Some(SyntaxRole::Attribute), false),
                (":".to_owned(), None, true),
                ("true".to_owned(), Some(SyntaxRole::Keyword), false),
            ]
        );
    }
}
