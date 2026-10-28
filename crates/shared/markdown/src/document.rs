use std::mem;

use crate::block::{Block, BlockKind};
use crate::footnotes::{self, FootnoteIndex};
use crate::frontmatter::Frontmatter;
use crate::line_kind::{self, LineKind};
use crate::link_ref::LinkDefinitions;
use crate::parse;
use crate::span::{Span, TextEdit};
use crate::table::TableModel;
use crate::text::{Line, is_blank, line_at, lines_of};

#[derive(Clone, Debug, Default)]
pub struct Document {
    text: String,
    blocks: Vec<Block>,
    lines: Vec<Line>,
    kinds: Vec<LineKind>,
    definitions: LinkDefinitions,
}

impl Document {
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let lines = lines_of(text);
        let parsed = parse::parse(text, &lines, true);
        let kinds = line_kind::build(text, &lines, &parsed.blocks);
        Self {
            text: text.to_owned(),
            blocks: parsed.blocks,
            lines,
            kinds,
            definitions: parsed.definitions,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    #[must_use]
    pub const fn definitions(&self) -> &LinkDefinitions {
        &self.definitions
    }

    #[must_use]
    pub fn frontmatter(&self) -> Option<&Frontmatter> {
        self.blocks.iter().find_map(Block::frontmatter)
    }

    #[must_use]
    pub fn tables(&self) -> Vec<&TableModel> {
        let mut tables = Vec::new();
        for block in &self.blocks {
            collect_tables(block, &mut tables);
        }
        tables
    }

    #[must_use]
    pub fn footnotes(&self) -> FootnoteIndex {
        footnotes::build(&self.blocks)
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn line_kind(&self, line: usize) -> LineKind {
        self.kinds.get(line).copied().unwrap_or(LineKind::Blank)
    }

    #[must_use]
    pub fn line_text(&self, line: usize) -> &str {
        self.lines
            .get(line)
            .map_or("", |entry| entry.text(&self.text))
    }

    #[must_use]
    pub fn line_span(&self, line: usize) -> Span {
        self.lines
            .get(line)
            .map_or_else(|| Span::empty(self.text.len()), Line::span)
    }

    #[must_use]
    pub fn line_of(&self, offset: usize) -> usize {
        line_at(&self.lines, offset)
    }

    fn blank_run_end(&self, end: usize) -> usize {
        let mut probe = line_at(&self.lines, end) + 1;
        let mut end = end;
        while let Some(line) = self.lines.get(probe) {
            if !is_blank(line.text(&self.text)) {
                break;
            }
            end = line.end;
            probe += 1;
        }
        end
    }

    #[must_use]
    pub fn block_at(&self, offset: usize) -> Option<&Block> {
        for block in &self.blocks {
            if let Some(found) = descend(block, offset) {
                return Some(found);
            }
        }
        None
    }

    #[must_use]
    pub fn reparsed(mut self, edit: &TextEdit) -> (Self, String) {
        let text = edit.applied(&self.text);
        let old_start = edit.span.start.min(self.text.len());
        let old_end = edit.span.end.min(self.text.len());
        let first = self
            .blocks
            .iter()
            .position(|block| block.span.end >= old_start)
            .unwrap_or(self.blocks.len());
        let last = self
            .blocks
            .iter()
            .rposition(|block| block.span.start <= old_end)
            .map_or(0, |index| index + 1);
        let from = first.saturating_sub(1);
        let mut to = (last + 1).min(self.blocks.len());
        let line_start = self
            .lines
            .get(line_at(&self.lines, old_start))
            .map_or(0, |line| line.start);
        let line_end = self
            .lines
            .get(line_at(&self.lines, old_end))
            .map_or(self.text.len(), |line| line.end);
        let block_start = self
            .blocks
            .get(from)
            .map_or(usize::MAX, |block| block.span.start);
        let region_start = block_start.min(line_start);
        let delta = isize::try_from(text.len()).expect("note text fits in isize")
            - isize::try_from(self.text.len()).expect("note text fits in isize");
        let new_lines = lines_of(&text);
        let mut whole_suffix = false;
        let parsed = loop {
            let block_end = if to > from {
                self.blocks.get(to - 1).map_or(0, |block| block.span.end)
            } else {
                0
            };
            let region_end = if whole_suffix {
                self.text.len()
            } else {
                self.blank_run_end(block_end.max(line_end))
            };
            let new_end = region_end
                .checked_add_signed(delta)
                .expect("the edited region stays inside the text");
            let region_lines: Vec<Line> = new_lines
                .iter()
                .copied()
                .filter(|line| line.start >= region_start && line.end <= new_end)
                .collect();
            let candidate = parse::parse(&text, &region_lines, region_start == 0);
            if !candidate.blocks.last().is_some_and(may_extend) || whole_suffix {
                break candidate;
            }
            whole_suffix = true;
            to = self.blocks.len();
        };
        let old_labels: Vec<&str> = self
            .blocks
            .get(from..to)
            .unwrap_or_default()
            .iter()
            .filter_map(|block| match &block.kind {
                BlockKind::LinkReference { label } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        let mut changed = false;
        for (label, target) in parsed.definitions.entries() {
            if self.definitions.get_normalized(label) != Some(target) {
                changed = true;
            }
        }
        if old_labels
            .iter()
            .any(|label| parsed.definitions.get_normalized(label).is_none())
        {
            changed = true;
        }
        if changed {
            return (Self::parse(&text), self.text);
        }
        let suffix = self.blocks.split_off(to.max(from).min(self.blocks.len()));
        self.blocks.truncate(from);
        self.blocks.extend(parsed.blocks);
        for mut block in suffix {
            block.shift(delta);
            self.blocks.push(block);
        }
        self.kinds = line_kind::build(&text, &new_lines, &self.blocks);
        self.lines = new_lines;
        let previous = mem::replace(&mut self.text, text);
        (self, previous)
    }
}

fn descend(block: &Block, offset: usize) -> Option<&Block> {
    if !block.span.contains(offset) {
        return None;
    }
    for child in &block.children {
        if let Some(found) = descend(child, offset) {
            return Some(found);
        }
    }
    Some(block)
}

fn may_extend(block: &Block) -> bool {
    match &block.kind {
        BlockKind::CodeBlock { fenced: false, .. } | BlockKind::FootnoteDefinition { .. } => true,
        BlockKind::CodeBlock { fenced: true, .. }
        | BlockKind::MathBlock { single_line: false }
        | BlockKind::Comment => block.markers.len() < 2,
        _ => false,
    }
}

fn collect_tables<'a>(block: &'a Block, tables: &mut Vec<&'a TableModel>) {
    if let Some(table) = block.table() {
        tables.push(table);
    }
    for child in &block.children {
        collect_tables(child, tables);
    }
}
