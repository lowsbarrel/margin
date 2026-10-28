use margin_markdown::{Block, BlockKind, Document, HeadingStyle};

use crate::block_path::{first_line, last_line, lines_span};
use crate::callout_folds::CalloutFolds;
use crate::line_view::Mode;
use crate::row::{Reveal, Row, RowKind};

#[derive(Debug)]
struct Special {
    first: usize,
    last: usize,
    kind: Option<RowKind>,
}

#[derive(Debug)]
struct Planner<'a> {
    document: &'a Document,
    touched_lines: Vec<usize>,
    folds: &'a CalloutFolds,
    specials: Vec<Special>,
}

#[must_use]
pub fn plan_rows(
    document: &Document,
    mode: Mode,
    reveal: &Reveal,
    folds: &CalloutFolds,
) -> Vec<Row> {
    let count = document.line_count();
    let mut rows = Vec::with_capacity(count);
    let specials = match mode {
        Mode::Raw => Vec::new(),
        Mode::LivePreview => {
            let mut touched_lines: Vec<usize> = reveal
                .points
                .iter()
                .map(|point| document.line_of(*point))
                .collect();
            touched_lines.sort_unstable();
            touched_lines.dedup();
            let mut planner = Planner {
                document,
                touched_lines,
                folds,
                specials: Vec::new(),
            };
            planner.collect(document.blocks(), 0);
            planner.specials.sort_by_key(|special| special.first);
            planner.specials
        }
    };
    let mut specials = specials.into_iter().peekable();
    let mut line = 0;
    while line < count {
        while specials.next_if(|special| special.first < line).is_some() {}
        if let Some(special) = specials.next_if(|special| special.first == line) {
            if let Some(kind) = special.kind {
                rows.push(Row {
                    kind,
                    lines: special.first..special.last + 1,
                    span: lines_span(document, special.first, special.last),
                });
            }
            line = special.last + 1;
            continue;
        }
        rows.push(Row {
            kind: RowKind::Line,
            lines: line..line + 1,
            span: document.line_span(line),
        });
        line += 1;
    }
    rows
}

impl Planner<'_> {
    fn touched(&self, first: usize, last: usize) -> bool {
        let at = self.touched_lines.partition_point(|line| *line < first);
        self.touched_lines.get(at).is_some_and(|line| *line <= last)
    }

    fn push(&mut self, first: usize, last: usize, kind: Option<RowKind>) {
        self.specials.push(Special { first, last, kind });
    }

    fn collect(&mut self, blocks: &[Block], depth: usize) {
        for block in blocks {
            let first = first_line(self.document, block.span);
            let last = last_line(self.document, block.span);
            match &block.kind {
                BlockKind::Frontmatter(frontmatter) if !frontmatter.unsupported => {
                    self.push(first, last, Some(RowKind::Properties));
                    continue;
                }
                BlockKind::Table(table) if depth == 0 => {
                    let last = last.max(self.document.line_of(table.delimiter.end));
                    self.push(first, last, Some(RowKind::Table));
                    continue;
                }
                BlockKind::MathBlock { single_line } if !self.touched(first, last) => {
                    let tex = math_tex(self.document.text(), block, *single_line);
                    self.push(first, last, Some(RowKind::MathBlock { tex }));
                    continue;
                }
                BlockKind::CodeBlock { fenced: true, .. }
                    if is_mermaid(block) && !self.touched(first, last) =>
                {
                    let source = trim_break(block.content.slice(self.document.text())).to_owned();
                    self.push(first, last, Some(RowKind::Mermaid { source }));
                    continue;
                }
                BlockKind::Callout { fold, .. }
                    if last > first
                        && !self.touched(first, last)
                        && self.folds.is_collapsed(block.span.start, *fold) =>
                {
                    self.push(first + 1, last, None);
                    continue;
                }
                BlockKind::ColonCallout { .. }
                    if block.markers.len() > 1 && !self.touched(first, last) =>
                {
                    self.collect(&block.children, depth + 1);
                    self.push(last, last, None);
                    continue;
                }
                BlockKind::Heading {
                    style: HeadingStyle::Setext,
                    ..
                } => {
                    if !self.touched(last, last) {
                        self.push(last, last, None);
                    }
                    continue;
                }
                _ => {}
            }
            self.collect(&block.children, depth + 1);
        }
    }
}

fn is_mermaid(block: &Block) -> bool {
    block
        .language()
        .is_some_and(|language| language.eq_ignore_ascii_case("mermaid"))
}

fn trim_break(text: &str) -> &str {
    text.strip_suffix('\n')
        .map_or(text, |text| text.strip_suffix('\r').unwrap_or(text))
}

fn math_tex(source: &str, block: &Block, single_line: bool) -> String {
    if single_line || block.markers.len() > 1 {
        return trim_break(block.content.slice(source)).to_owned();
    }
    block.span.slice(source).replace('$', "").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use margin_markdown::Fold;

    use super::*;

    fn kinds(text: &str, points: &[usize], folds: &CalloutFolds) -> Vec<(RowKind, usize, usize)> {
        let document = Document::parse(text);
        plan_rows(
            &document,
            Mode::LivePreview,
            &Reveal::new(points.to_vec()),
            folds,
        )
        .into_iter()
        .map(|row| (row.kind, row.lines.start, row.lines.end))
        .collect()
    }

    #[test]
    fn multi_line_blocks_become_single_rows() {
        let text = "---\na: 1\n---\n| a | b |\n| - | - |\n$$\nx\n$$\n```mermaid\ngraph\n```\nend";
        assert_eq!(
            kinds(text, &[], &CalloutFolds::new()),
            vec![
                (RowKind::Properties, 0, 3),
                (RowKind::Table, 3, 5),
                (
                    RowKind::MathBlock {
                        tex: "x".to_owned()
                    },
                    5,
                    8
                ),
                (
                    RowKind::Mermaid {
                        source: "graph".to_owned()
                    },
                    8,
                    11
                ),
                (RowKind::Line, 11, 12),
            ]
        );
        let caret = text.find("x\n").expect("math body");
        let rows = kinds(text, &[caret], &CalloutFolds::new());
        assert_eq!(rows.get(2), Some(&(RowKind::Line, 5, 6)));
        let document = Document::parse(text);
        let raw = plan_rows(
            &document,
            Mode::Raw,
            &Reveal::default(),
            &CalloutFolds::new(),
        );
        assert_eq!(raw.len(), document.line_count());
    }

    #[test]
    fn folded_callouts_drop_their_body() {
        let text = "> [!note]- T\n> a\n> b\ntext";
        let mut folds = CalloutFolds::new();
        assert_eq!(
            kinds(text, &[], &folds),
            vec![(RowKind::Line, 0, 1), (RowKind::Line, 3, 4)]
        );
        assert_eq!(kinds(text, &[14], &folds).len(), 4);
        folds.toggle(0, Fold::Collapsed);
        assert_eq!(kinds(text, &[], &folds).len(), 4);
    }

    #[test]
    fn hidden_marker_lines_produce_no_rows() {
        let colon = ":::note\nbody\n:::\nafter";
        assert_eq!(
            kinds(colon, &[], &CalloutFolds::new()),
            vec![
                (RowKind::Line, 0, 1),
                (RowKind::Line, 1, 2),
                (RowKind::Line, 3, 4)
            ]
        );
        assert_eq!(kinds(colon, &[9], &CalloutFolds::new()).len(), 4);
        let setext = "Title\n=====\nbody";
        assert_eq!(kinds(setext, &[], &CalloutFolds::new()).len(), 2);
        assert_eq!(kinds(setext, &[8], &CalloutFolds::new()).len(), 3);
    }
}
