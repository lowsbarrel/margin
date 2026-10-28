use std::time::Duration;
use std::{mem, slice};

use margin_markdown::{Document, Span, TextEdit};

use crate::change::Change;
use crate::edit_origin::EditOrigin;
use crate::history::{Entry, History};
use crate::normalize::{self, Normalized};
use crate::selection::Selection;
use crate::{compose, mapping};

#[derive(Debug)]
pub struct Buffer {
    document: Document,
    selections: Normalized,
    revision: u64,
    history: History,
}

impl Buffer {
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            document: Document::parse(text),
            selections: Normalized {
                selections: vec![Selection::cursor(0)],
                primary: 0,
            },
            revision: 0,
            history: History::default(),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        self.document.text()
    }

    #[must_use]
    pub const fn document(&self) -> &Document {
        &self.document
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn selections(&self) -> &[Selection] {
        &self.selections.selections
    }

    #[must_use]
    pub const fn primary_index(&self) -> usize {
        self.selections.primary
    }

    #[must_use]
    pub fn primary(&self) -> Selection {
        self.selections.selections[self.selections.primary]
    }

    pub fn set_selections(&mut self, selections: Vec<Selection>, primary: usize) {
        self.selections = normalize::normalize(self.document.text(), selections, primary);
    }

    pub fn select(&mut self, selection: Selection) {
        self.set_selections(vec![selection], 0);
    }

    pub fn add_selection(&mut self, selection: Selection) {
        let mut selections = self.selections.selections.clone();
        selections.push(selection);
        let primary = selections.len() - 1;
        self.set_selections(selections, primary);
    }

    pub fn apply(&mut self, change: Change, origin: EditOrigin, now: Duration) -> Option<Span> {
        let Change { edits, selections } = change;
        let edits = compose::sorted_and_validated(self.text(), edits);
        let after = selections.unwrap_or_else(|| self.mapped_selections(&edits));
        let primary = self.selections.primary;
        let Some(edit) = compose::composed(self.text(), edits)
            .filter(|edit| edit.span.slice(self.text()) != edit.insert)
        else {
            self.set_selections(after, primary);
            self.history.break_group();
            return None;
        };
        let before = self.selections.clone();
        let old = edit.span.slice(self.text()).to_owned();
        let previous = self.replace_document(&edit);
        self.set_selections(after, primary);
        let changed = Span::new(edit.span.start, edit.span.start + edit.insert.len());
        let entry = Entry {
            start: edit.span.start,
            old,
            new: edit.insert,
            before,
            after: self.selections.clone(),
        };
        self.history.record(entry, origin, now, previous.text());
        Some(changed)
    }

    pub fn undo(&mut self) -> Option<Span> {
        let restored = self.history.undo(&self.document)?;
        self.document = restored.document;
        self.selections = restored.selections;
        self.revision += 1;
        Some(restored.changed)
    }

    pub fn redo(&mut self) -> Option<Span> {
        let restored = self.history.redo(&self.document)?;
        self.document = restored.document;
        self.selections = restored.selections;
        self.revision += 1;
        Some(restored.changed)
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn break_undo_group(&mut self) {
        self.history.break_group();
    }

    pub fn replace_all(&mut self, text: &str) -> Option<Span> {
        let current = self.text();
        if current == text {
            return None;
        }
        let common_prefix = current
            .bytes()
            .zip(text.bytes())
            .take_while(|(left, right)| left == right)
            .count();
        let prefix = current.floor_char_boundary(common_prefix);
        let longest_suffix = current.len().min(text.len()) - prefix;
        let mut suffix = current
            .bytes()
            .rev()
            .zip(text.bytes().rev())
            .take(longest_suffix)
            .take_while(|(left, right)| left == right)
            .count();
        while !current.is_char_boundary(current.len() - suffix) {
            suffix -= 1;
        }
        let edit = TextEdit::new(
            Span::new(prefix, current.len() - suffix),
            &text[prefix..text.len() - suffix],
        );
        let after = self.mapped_selections(slice::from_ref(&edit));
        self.replace_document(&edit);
        self.set_selections(after, self.selections.primary);
        self.history.clear();
        Some(Span::new(prefix, prefix + edit.insert.len()))
    }

    fn mapped_selections(&self, edits: &[TextEdit]) -> Vec<Selection> {
        self.selections
            .selections
            .iter()
            .map(|selection| mapping::map_selection(edits, *selection))
            .collect()
    }

    fn replace_document(&mut self, edit: &TextEdit) -> Document {
        let document = self.document.reparse(edit);
        self.revision += 1;
        mem::replace(&mut self.document, document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(start: usize, end: usize, insert: &str) -> TextEdit {
        TextEdit::new(Span::new(start, end), insert)
    }

    fn apply(buffer: &mut Buffer, edits: Vec<TextEdit>) -> Option<Span> {
        buffer.apply(Change::new(edits), EditOrigin::Command, Duration::ZERO)
    }

    #[test]
    fn a_new_buffer_has_one_cursor_at_the_start() {
        let buffer = Buffer::new("# Title\n");
        assert_eq!(buffer.text(), "# Title\n");
        assert_eq!(buffer.document().text(), buffer.text());
        assert_eq!(buffer.selections(), [Selection::cursor(0)]);
        assert_eq!(buffer.revision(), 0);
        assert!(!buffer.can_undo());
    }

    #[test]
    fn single_edit_reparses_and_reports_the_changed_region() {
        let mut buffer = Buffer::new("hello\n\nworld\n");
        assert_eq!(
            apply(&mut buffer, vec![edit(0, 0, "# ")]),
            Some(Span::new(0, 2))
        );
        assert_eq!(buffer.text(), "# hello\n\nworld\n");
        assert_eq!(buffer.revision(), 1);
    }

    #[test]
    fn several_edits_apply_as_one_region() {
        let mut buffer = Buffer::new("a b c");
        let changed = apply(&mut buffer, vec![edit(4, 5, "**c**"), edit(0, 1, "**a**")]);
        assert_eq!(buffer.text(), "**a** b **c**");
        assert_eq!(changed, Some(Span::new(0, 13)));
        assert_eq!(buffer.revision(), 1);
    }

    #[test]
    fn cursors_move_after_text_inserted_at_them() {
        let mut buffer = Buffer::new("ab");
        buffer.set_selections(vec![Selection::cursor(0), Selection::cursor(2)], 1);
        apply(&mut buffer, vec![edit(0, 0, "x"), edit(2, 2, "y")]);
        assert_eq!(buffer.text(), "xaby");
        assert_eq!(
            buffer.selections(),
            [Selection::cursor(1), Selection::cursor(4)]
        );
        assert_eq!(buffer.primary(), Selection::cursor(4));
    }

    #[test]
    fn explicit_selections_win_over_mapping() {
        let mut buffer = Buffer::new("word");
        let change =
            Change::new(vec![edit(0, 4, "**word**")]).with_selections(vec![Selection::new(2, 6)]);
        buffer.apply(change, EditOrigin::Command, Duration::ZERO);
        assert_eq!(buffer.selections(), [Selection::new(2, 6)]);
    }

    #[test]
    fn selection_only_changes_touch_neither_text_nor_history() {
        let mut buffer = Buffer::new("abc");
        let change = Change::default().with_selections(vec![Selection::new(0, 3)]);
        assert_eq!(
            buffer.apply(change, EditOrigin::Command, Duration::ZERO),
            None
        );
        assert_eq!(buffer.selections(), [Selection::new(0, 3)]);
        assert_eq!(buffer.revision(), 0);
        assert!(!buffer.can_undo());
        assert_eq!(apply(&mut buffer, vec![edit(0, 1, "a")]), None);
        assert_eq!(buffer.revision(), 0);
    }

    #[test]
    fn add_selection_becomes_primary_through_merging() {
        let mut buffer = Buffer::new("abcdef");
        buffer.select(Selection::new(4, 6));
        buffer.add_selection(Selection::cursor(1));
        assert_eq!(buffer.primary_index(), 0);
        buffer.add_selection(Selection::new(5, 3));
        assert_eq!(
            buffer.selections(),
            [Selection::cursor(1), Selection::new(3, 6)]
        );
        assert_eq!(buffer.primary_index(), 1);
    }

    #[test]
    fn replace_all_adopts_text_with_one_minimal_edit() {
        let mut buffer = Buffer::new("one two three");
        buffer.select(Selection::cursor(13));
        apply(&mut buffer, vec![edit(0, 0, "x")]);
        assert!(buffer.can_undo());
        assert_eq!(buffer.replace_all("xone 2 three"), Some(Span::new(5, 6)));
        assert_eq!(buffer.text(), "xone 2 three");
        assert_eq!(buffer.selections(), [Selection::cursor(12)]);
        assert!(!buffer.can_undo());
        assert_eq!(buffer.revision(), 2);
        assert_eq!(buffer.replace_all("xone 2 three"), None);
    }

    #[test]
    fn replace_all_keeps_edits_on_character_boundaries_and_clamps() {
        let mut buffer = Buffer::new("aéz long tail");
        buffer.select(Selection::cursor(13));
        assert_eq!(buffer.replace_all("aéz"), Some(Span::empty(4)));
        assert_eq!(buffer.selections(), [Selection::cursor(4)]);
        assert_eq!(buffer.replace_all("a©z"), Some(Span::new(1, 3)));
        assert_eq!(buffer.text(), "a©z");
        assert_eq!(buffer.replace_all(""), Some(Span::empty(0)));
        assert_eq!(buffer.selections(), [Selection::cursor(0)]);
    }
}
