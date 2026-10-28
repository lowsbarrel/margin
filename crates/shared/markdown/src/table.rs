use std::iter;

use crate::inline::Inline;
use crate::parse::starts_block;
use crate::span::Span;
use crate::text::Line;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
    None,
}

impl Align {
    #[must_use]
    pub const fn css(self) -> Option<&'static str> {
        match self {
            Self::Left => Some("left"),
            Self::Center => Some("center"),
            Self::Right => Some("right"),
            Self::None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellPlace {
    pub row: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableCell {
    pub text: String,
    pub span: Span,
    pub slot: Span,
    pub inlines: Vec<Inline>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRow {
    pub span: Span,
    pub cells: Vec<TableCell>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableModel {
    pub span: Span,
    pub delimiter: Span,
    pub cols: usize,
    pub align: Vec<Align>,
    pub rows: Vec<TableRow>,
    pub anchored: bool,
}

impl TableModel {
    #[must_use]
    pub fn text_rows(&self) -> Vec<Vec<String>> {
        self.rows
            .iter()
            .map(|row| row.cells.iter().map(|cell| cell.text.clone()).collect())
            .collect()
    }

    #[must_use]
    pub fn locate_cell(&self, offset: usize) -> Option<CellPlace> {
        for (row, line) in self.rows.iter().enumerate() {
            if offset < line.span.start || offset > line.span.end {
                continue;
            }
            for (col, cell) in line.cells.iter().enumerate() {
                if offset <= cell.slot.end {
                    return Some(CellPlace { row, col });
                }
            }
            return Some(CellPlace {
                row,
                col: line.cells.len().saturating_sub(1),
            });
        }
        None
    }

    #[must_use]
    pub fn cell(&self, place: CellPlace) -> Option<&TableCell> {
        self.rows.get(place.row)?.cells.get(place.col)
    }
}

#[must_use]
pub fn empty_row(cols: usize) -> Vec<String> {
    vec![String::new(); cols]
}

#[must_use]
pub fn display_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some(next @ ('\\' | '|')) => out.push(next),
                Some(next) => {
                    out.push('\\');
                    out.push(next);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[must_use]
pub(crate) fn slots_of(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut pipes = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        match bytes.get(at) {
            Some(b'\\') => at += 2,
            Some(b'|') => {
                pipes.push(at);
                at += 1;
            }
            Some(_) => at += 1,
            None => break,
        }
    }
    let start = text.len() - text.trim_start().len();
    let end = text.trim_end().len();
    let mut bounds: Vec<Option<usize>> = pipes.iter().copied().map(Some).collect();
    if pipes.first() != Some(&start) {
        bounds.insert(0, if start == 0 { None } else { Some(start - 1) });
    }
    if pipes.last() != Some(&end.saturating_sub(1)) || pipes.is_empty() {
        bounds.push(Some(end));
    }
    let mut slots = Vec::new();
    for pair in bounds.windows(2) {
        if let [from, to] = pair {
            slots.push((from.map_or(0, |value| value + 1), to.unwrap_or(0)));
        }
    }
    slots
}

fn cells_of(source: &str, line: Line) -> Vec<TableCell> {
    let text = line.text(source);
    slots_of(text)
        .into_iter()
        .map(|(from, to)| {
            let raw = text.get(from..to).unwrap_or_default();
            let lead = raw.len() - raw.trim_start().len();
            let trimmed = raw.trim();
            let start = if trimmed.is_empty() {
                let base = if raw.starts_with(' ') { from + 1 } else { from };
                base.min(to)
            } else {
                from + lead
            };
            TableCell {
                text: trimmed.to_owned(),
                span: Span::new(line.start + start, line.start + start + trimmed.len()),
                slot: Span::new(line.start + from, line.start + to),
                inlines: Vec::new(),
            }
        })
        .collect()
}

fn delimiter_shape(text: &str) -> Option<(bool, bool)> {
    let value = text.trim();
    let (lead, rest) = match value.strip_prefix(':') {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    let (trail, body) = match rest.strip_suffix(':') {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    if body.is_empty() || !body.bytes().all(|byte| byte == b'-') {
        return None;
    }
    Some((lead, trail))
}

#[must_use]
pub fn align_of(text: &str) -> Align {
    match delimiter_shape(text) {
        Some((true, true)) => Align::Center,
        Some((true, false)) => Align::Left,
        Some((false, true)) => Align::Right,
        _ => Align::None,
    }
}

fn is_delimiter_row(text: &str) -> bool {
    let slots = slots_of(text);
    !slots.is_empty()
        && slots
            .iter()
            .all(|(from, to)| delimiter_shape(text.get(*from..*to).unwrap_or_default()).is_some())
}

pub(crate) fn parse(
    source: &str,
    lines: &[Line],
    index: usize,
    anchored: bool,
) -> Option<(TableModel, usize)> {
    let header = lines.get(index)?;
    let header_text = header.text(source);
    if !header_text.contains('|') {
        return None;
    }
    let delimiter = lines.get(index + 1)?;
    let delimiter_text = delimiter.text(source);
    if !delimiter_text.contains('|') || !is_delimiter_row(delimiter_text) {
        return None;
    }
    let align: Vec<Align> = slots_of(delimiter_text)
        .iter()
        .map(|(from, to)| align_of(delimiter_text.get(*from..*to).unwrap_or_default()))
        .collect();
    let mut rows = vec![TableRow {
        span: header.span(),
        cells: cells_of(source, *header),
    }];
    let mut next = index + 2;
    while let Some(line) = lines.get(next) {
        let text = line.text(source);
        if text.trim().is_empty() || !text.contains('|') || starts_block(text) {
            break;
        }
        rows.push(TableRow {
            span: line.span(),
            cells: cells_of(source, *line),
        });
        next += 1;
    }
    let cols = rows
        .iter()
        .map(|row| row.cells.len())
        .chain(iter::once(align.len()))
        .max()
        .unwrap_or(1)
        .max(1);
    let mut align = align;
    while align.len() < cols {
        align.push(Align::None);
    }
    align.truncate(cols);
    for row in &mut rows {
        row.cells.truncate(cols);
        let end = row.span.end;
        while row.cells.len() < cols {
            row.cells.push(TableCell {
                text: String::new(),
                span: Span::empty(end),
                slot: Span::empty(end),
                inlines: Vec::new(),
            });
        }
    }
    let last = rows.last().map_or_else(|| header.span(), |row| row.span);
    Some((
        TableModel {
            span: Span::new(header.start, last.end),
            delimiter: delimiter.span(),
            cols,
            align,
            rows,
            anchored,
        },
        next,
    ))
}
