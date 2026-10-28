use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::mapping::{self, Assoc};
use crate::text::Line;

struct LineBlock {
    from: usize,
    to: usize,
    last: usize,
    ranges: Vec<Selection>,
}

fn line_blocks(document: &Document, selections: &[Selection]) -> Vec<LineBlock> {
    let mut blocks: Vec<LineBlock> = Vec::new();
    for range in selections {
        let first = Line::at(document, range.start());
        let mut last = Line::at(document, range.end());
        if !range.is_empty() && range.end() == last.start && last.index > first.index {
            last = Line::at(document, range.end() - 1);
        }
        match blocks.last_mut() {
            Some(block) if block.last >= first.index => {
                block.to = last.end;
                block.last = last.index;
                block.ranges.push(*range);
            }
            _ => blocks.push(LineBlock {
                from: first.start,
                to: last.end,
                last: last.index,
                ranges: vec![*range],
            }),
        }
    }
    blocks
}

fn column_in(text: &str, column: usize) -> usize {
    text.char_indices()
        .nth(column)
        .map_or(text.len(), |(at, _)| at)
}

#[must_use]
pub fn delete_line(document: &Document, selections: &[Selection]) -> Change {
    let length = document.text().len();
    let blocks = line_blocks(document, selections);
    let mut edits = Vec::new();
    let mut targets = Vec::new();
    for block in &blocks {
        let (mut from, mut to) = (block.from, block.to);
        if from > 0 {
            from -= 1;
        } else if to < length {
            to += 1;
        }
        edits.push(TextEdit::new(Span::new(from, to), ""));
        for range in &block.ranges {
            let line = Line::at(document, range.head);
            let column = line
                .text
                .get(..range.head - line.start)
                .map_or(0, |head| head.chars().count());
            let target = if block.last + 1 < document.line_count() {
                let below = Line::numbered(document, block.last + 1);
                below.start + column_in(below.text, column)
            } else {
                length
            };
            targets.push(target);
        }
    }
    let edits = mapping::sorted(edits);
    let moved = targets
        .into_iter()
        .map(|target| Selection::cursor(mapping::map_pos(&edits, target, Assoc::Before)))
        .collect();
    Change::new(edits).with_selections(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::run;

    #[test]
    fn deletes_whole_lines_and_keeps_the_column() {
        assert_eq!(
            run("one\ntw|o\nthree", |d, s| Some(delete_line(d, s))).as_deref(),
            Some("one\nth|ree")
        );
        assert_eq!(
            run("one\ntw|o", |d, s| Some(delete_line(d, s))).as_deref(),
            Some("one|")
        );
        assert_eq!(
            run("o|ne\ntwo", |d, s| Some(delete_line(d, s))).as_deref(),
            Some("t|wo")
        );
    }
}
