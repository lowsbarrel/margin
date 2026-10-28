use crate::table::{Align, TableModel};

const WIDE_RANGES: &[(u32, u32)] = &[
    (0x1100, 0x115f),
    (0x2e80, 0xa4cf),
    (0xac00, 0xd7a3),
    (0xf900, 0xfaff),
    (0xfe30, 0xfe4f),
    (0xff00, 0xff60),
    (0xffe0, 0xffe6),
    (0x1f300, 0x1faff),
    (0x20000, 0x3fffd),
];

#[must_use]
pub fn is_wide(ch: char) -> bool {
    let code = u32::from(ch);
    WIDE_RANGES
        .iter()
        .any(|(from, to)| code >= *from && code <= *to)
}

#[must_use]
pub fn width_of(text: &str) -> usize {
    text.chars().map(|ch| if is_wide(ch) { 2 } else { 1 }).sum()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormattedTable {
    pub lines: Vec<String>,
    pub starts: Vec<Vec<usize>>,
}

struct Padded {
    text: String,
    lead: usize,
}

fn padded(text: &str, width: usize, align: Align) -> Padded {
    let gap = width.saturating_sub(width_of(text));
    let lead = match align {
        Align::Right => gap,
        Align::Center => gap / 2,
        _ => 0,
    };
    let mut out = String::with_capacity(text.len() + gap);
    for _ in 0..lead {
        out.push(' ');
    }
    out.push_str(text);
    for _ in 0..gap - lead {
        out.push(' ');
    }
    Padded { text: out, lead }
}

fn delimiter(width: usize, align: Align) -> String {
    let mut out = String::with_capacity(width);
    match align {
        Align::Left => {
            out.push(':');
            for _ in 1..width {
                out.push('-');
            }
        }
        Align::Right => {
            for _ in 1..width {
                out.push('-');
            }
            out.push(':');
        }
        Align::Center => {
            out.push(':');
            for _ in 2..width {
                out.push('-');
            }
            out.push(':');
        }
        Align::None => {
            for _ in 0..width {
                out.push('-');
            }
        }
    }
    out
}

#[must_use]
pub fn format_table(rows: &[Vec<String>], align: &[Align]) -> FormattedTable {
    let cols = align.len();
    let mut widths = vec![3_usize; cols];
    for row in rows {
        for (col, width) in widths.iter_mut().enumerate() {
            let cell = row.get(col).map_or("", String::as_str);
            *width = (*width).max(width_of(cell));
        }
    }
    let mut lines = Vec::with_capacity(rows.len() + 1);
    let mut starts = Vec::with_capacity(rows.len());
    for row in rows {
        let mut at = 2;
        let mut cells = Vec::with_capacity(cols);
        let mut row_starts = Vec::with_capacity(cols);
        for col in 0..cols {
            let cell = row.get(col).map_or("", String::as_str);
            let padded = padded(
                cell,
                widths.get(col).copied().unwrap_or(3),
                align.get(col).copied().unwrap_or(Align::None),
            );
            row_starts.push(at + padded.lead);
            at += padded.text.len() + 3;
            cells.push(padded.text);
        }
        starts.push(row_starts);
        let mut line = String::from("| ");
        line.push_str(&cells.join(" | "));
        line.push_str(" |");
        lines.push(line);
    }
    let rule = {
        let cells: Vec<String> = widths
            .iter()
            .enumerate()
            .map(|(col, width)| delimiter(*width, align.get(col).copied().unwrap_or(Align::None)))
            .collect();
        let mut line = String::from("| ");
        line.push_str(&cells.join(" | "));
        line.push_str(" |");
        line
    };
    if lines.is_empty() {
        lines.push(rule);
    } else {
        lines.insert(1, rule);
    }
    FormattedTable { lines, starts }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRewrite {
    pub text: String,
    pub cell_starts: Vec<Vec<usize>>,
}

fn leading_whitespace(line: &str) -> &str {
    let trimmed = line.trim_start_matches([' ', '\t']);
    line.get(..line.len() - trimmed.len()).unwrap_or_default()
}

#[must_use]
pub fn rewrite(
    table: &TableModel,
    source: &str,
    rows: &[Vec<String>],
    align: &[Align],
) -> TableRewrite {
    let formatted = format_table(rows, align);
    let old: Vec<&str> = table.span.slice(source).split('\n').collect();
    let indents: Vec<&str> = old.iter().map(|line| leading_whitespace(line)).collect();
    let fallback = indents.first().copied().unwrap_or_default();
    let mut lines = Vec::with_capacity(formatted.lines.len());
    let mut line_starts = Vec::with_capacity(formatted.lines.len());
    let mut at = 0;
    for (index, line) in formatted.lines.iter().enumerate() {
        let indent = indents.get(index).copied().unwrap_or(fallback);
        let mut out = String::with_capacity(indent.len() + line.len());
        out.push_str(indent);
        out.push_str(line);
        line_starts.push(at);
        at += out.len() + 1;
        lines.push(out);
    }
    let mut cell_starts = Vec::with_capacity(formatted.starts.len());
    for (row, row_starts) in formatted.starts.iter().enumerate() {
        let index = if row == 0 { 0 } else { row + 1 };
        let indent = indents.get(index).copied().unwrap_or(fallback);
        let base = line_starts.get(index).copied().unwrap_or(0) + indent.len();
        cell_starts.push(row_starts.iter().map(|start| base + start).collect());
    }
    TableRewrite {
        text: lines.join("\n"),
        cell_starts,
    }
}
