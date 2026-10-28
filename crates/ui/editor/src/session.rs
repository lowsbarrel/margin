use std::ops::Range;
use std::time::{Duration, Instant};

use gpui_kit::{ListAlignment, ListState, px};
use margin_live_preview::{CalloutFolds, Mode, Reveal, Row, plan_rows};
use margin_markdown::{Document, Fold, Span, TextEdit};
use margin_text_buffer::{Buffer, Change, EditOrigin, Selection};

pub(crate) const HEADER_ROWS: usize = 1;

pub(crate) struct Session {
    buffer: Buffer,
    mode: Mode,
    folds: CalloutFolds,
    frozen: Option<Vec<usize>>,
    reveal: Reveal,
    rows: Vec<Row>,
    list: ListState,
    started: Instant,
}

impl Session {
    pub(crate) fn new(text: &str, mode: Mode) -> Self {
        let buffer = Buffer::new(text);
        let folds = CalloutFolds::default();
        let reveal = carets(buffer.selections());
        let rows = plan_rows(buffer.document(), mode, &reveal, &folds);
        let list = ListState::new(rows.len() + HEADER_ROWS, ListAlignment::Top, px(600.0));
        Self {
            buffer,
            mode,
            folds,
            frozen: None,
            reveal,
            rows,
            list,
            started: Instant::now(),
        }
    }

    pub(crate) const fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub(crate) fn document(&self) -> &Document {
        self.buffer.document()
    }

    pub(crate) fn text(&self) -> &str {
        self.buffer.text()
    }

    pub(crate) fn selections(&self) -> &[Selection] {
        self.buffer.selections()
    }

    pub(crate) fn primary(&self) -> Selection {
        self.buffer.primary()
    }

    pub(crate) const fn mode(&self) -> Mode {
        self.mode
    }

    pub(crate) const fn reveal(&self) -> &Reveal {
        &self.reveal
    }

    pub(crate) const fn folds(&self) -> &CalloutFolds {
        &self.folds
    }

    pub(crate) fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub(crate) const fn list(&self) -> &ListState {
        &self.list
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub(crate) fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.refresh(Some(Span::new(0, self.text().len())), 0);
    }

    pub(crate) fn apply(&mut self, change: Change, origin: EditOrigin) -> Option<Span> {
        let before = self.text().len();
        let now = self.elapsed();
        let changed = self.buffer.apply(change, origin, now);
        self.after_edit(changed, before);
        changed
    }

    pub(crate) fn undo(&mut self) -> Option<Span> {
        let before = self.text().len();
        let changed = self.buffer.undo();
        self.after_edit(changed, before);
        changed
    }

    pub(crate) fn redo(&mut self) -> Option<Span> {
        let before = self.text().len();
        let changed = self.buffer.redo();
        self.after_edit(changed, before);
        changed
    }

    pub(crate) fn replace_all(&mut self, text: &str) -> Option<Span> {
        let before = self.text().len();
        let changed = self.buffer.replace_all(text);
        self.after_edit(changed, before);
        changed
    }

    pub(crate) fn set_selections(&mut self, selections: Vec<Selection>, primary: usize) {
        self.buffer.set_selections(selections, primary);
        self.buffer.break_undo_group();
        self.refresh(None, 0);
    }

    pub(crate) fn select(&mut self, selection: Selection) {
        self.set_selections(vec![selection], 0);
    }

    pub(crate) fn add_selection(&mut self, selection: Selection) {
        self.buffer.add_selection(selection);
        self.buffer.break_undo_group();
        self.refresh(None, 0);
    }

    pub(crate) fn freeze_reveal(&mut self) {
        if self.frozen.is_none() {
            self.frozen = Some(self.reveal.points.clone());
        }
    }

    pub(crate) fn thaw_reveal(&mut self) {
        if self.frozen.take().is_some() {
            self.refresh(None, 0);
        }
    }

    pub(crate) fn toggle_fold(&mut self, block_start: usize, fold: Fold) {
        self.folds.toggle(block_start, fold);
        self.refresh(Some(Span::new(block_start, self.text().len())), 0);
    }

    pub(crate) fn row_at_offset(&self, offset: usize) -> Option<usize> {
        let index = self
            .rows
            .partition_point(|row| row.span.start <= offset)
            .checked_sub(1)?;
        Some(index + HEADER_ROWS)
    }

    pub(crate) fn row(&self, item: usize) -> Option<&Row> {
        self.rows.get(item.checked_sub(HEADER_ROWS)?)
    }

    pub(crate) fn reveal_primary(&self) {
        if let Some(item) = self.row_at_offset(self.primary().head) {
            self.list.scroll_to_reveal_item(item);
        }
    }

    fn after_edit(&mut self, changed: Option<Span>, before: usize) {
        if let Some(span) = changed {
            let delta = isize::try_from(self.text().len()).expect("note length fits in isize")
                - isize::try_from(before).expect("note length fits in isize");
            let old_end = span.end.saturating_add_signed(-delta).max(span.start);
            let inserted = span.slice(self.buffer.text()).to_owned();
            self.folds
                .map_through(&[TextEdit::new(Span::new(span.start, old_end), inserted)]);
            self.refresh(Some(span), delta);
        }
    }

    fn refresh(&mut self, changed: Option<Span>, delta: isize) {
        let old_points = shift_points(&self.reveal.points, changed, delta);
        self.reveal = self
            .frozen
            .clone()
            .map_or_else(|| carets(self.buffer.selections()), Reveal::new);
        let rows = plan_rows(self.buffer.document(), self.mode, &self.reveal, &self.folds);
        let prefix = self
            .rows
            .iter()
            .zip(&rows)
            .take_while(|(old, new)| old == new)
            .count();
        let limit = self.rows.len().min(rows.len()) - prefix;
        let suffix = self
            .rows
            .iter()
            .rev()
            .zip(rows.iter().rev())
            .take(limit)
            .take_while(|(old, new)| same_shape(old, new))
            .count();
        let mut dirty: Option<Range<usize>> = None;
        let mut mark = |index: usize| {
            dirty = Some(dirty.clone().map_or(index..index + 1, |range| {
                range.start.min(index)..range.end.max(index + 1)
            }));
        };
        if prefix < rows.len() - suffix || prefix < self.rows.len() - suffix {
            mark(prefix);
            mark((rows.len() - suffix).max(prefix + 1) - 1);
        }
        let touched = changed
            .into_iter()
            .chain(old_points.iter().map(|point| Span::empty(*point)))
            .chain(self.reveal.points.iter().map(|point| Span::empty(*point)));
        let old_count = self.rows.len();
        for span in touched {
            let first = rows.partition_point(|row| row.span.end < span.start);
            for (index, row) in rows.iter().enumerate().skip(first) {
                if row.span.start > span.end {
                    break;
                }
                mark(index);
            }
        }
        let new_count = rows.len();
        self.rows = rows;
        let Some(dirty) = dirty else {
            return;
        };
        let head = dirty.start.min(prefix).min(new_count);
        let tail = suffix.min(new_count - dirty.end.min(new_count));
        let old_end = old_count.saturating_sub(tail).max(head);
        let new_end = new_count - tail;
        self.list.splice(
            head + HEADER_ROWS..old_end + HEADER_ROWS,
            new_end.saturating_sub(head),
        );
    }
}

fn carets(selections: &[Selection]) -> Reveal {
    Reveal::new(
        selections
            .iter()
            .filter(|selection| selection.is_empty())
            .map(|selection| selection.head)
            .collect(),
    )
}

fn shift_points(points: &[usize], changed: Option<Span>, delta: isize) -> Vec<usize> {
    let Some(changed) = changed else {
        return points.to_vec();
    };
    let old_end = changed.end.saturating_add_signed(-delta).max(changed.start);
    points
        .iter()
        .map(|point| {
            if *point < changed.start {
                *point
            } else if *point > old_end {
                point.saturating_add_signed(delta)
            } else {
                changed.start
            }
        })
        .collect()
}

fn same_shape(old: &Row, new: &Row) -> bool {
    old.kind == new.kind && old.span.len() == new.span.len() && old.lines.len() == new.lines.len()
}
