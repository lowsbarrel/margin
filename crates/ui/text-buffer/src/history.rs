use std::collections::VecDeque;
use std::mem;
use std::time::Duration;

use margin_markdown::{Document, Span, TextEdit};

use crate::edit_origin::EditOrigin;
use crate::normalize::Normalized;

const GROUP_DELAY: Duration = Duration::from_millis(500);
const MAX_DEPTH: usize = 200;

#[derive(Debug)]
pub(crate) struct Entry {
    pub start: usize,
    pub old: String,
    pub new: String,
    pub before: Normalized,
    pub after: Normalized,
}

impl Entry {
    fn absorb(&mut self, next: Self, previous_text: &str) {
        let own_end = self.start + self.new.len();
        let start = self.start.min(next.start);
        let end = own_end.max(next.start + next.old.len());
        let prefix = &previous_text[start..self.start];
        let suffix = &previous_text[own_end..end];
        if !prefix.is_empty() {
            self.old.insert_str(0, prefix);
            self.new.insert_str(0, prefix);
        }
        self.old.push_str(suffix);
        self.new.push_str(suffix);
        let replaced = next.start - start..next.start - start + next.old.len();
        self.new.replace_range(replaced, &next.new);
        self.start = start;
        self.after = next.after;
    }
}

#[derive(Debug)]
pub(crate) struct Restored {
    pub document: Document,
    pub changed: Span,
    pub selections: Normalized,
}

#[derive(Debug, Default)]
pub(crate) struct History {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
    last_origin: Option<EditOrigin>,
    last_time: Duration,
    joinable: bool,
}

impl History {
    pub(crate) fn record(
        &mut self,
        entry: Entry,
        origin: EditOrigin,
        now: Duration,
        previous_text: &str,
    ) {
        self.redo.clear();
        let joins = self.joinable
            && self.last_origin == Some(origin)
            && matches!(origin, EditOrigin::Typing | EditOrigin::Deletion)
            && now.saturating_sub(self.last_time) < GROUP_DELAY;
        match self.undo.back_mut() {
            Some(last) if joins && last.after == entry.before => last.absorb(entry, previous_text),
            _ => {
                self.undo.push_back(entry);
                if self.undo.len() > MAX_DEPTH {
                    self.undo.pop_front();
                }
            }
        }
        self.last_origin = Some(origin);
        self.last_time = now;
        self.joinable = true;
    }

    pub(crate) fn break_group(&mut self) {
        self.joinable = false;
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(crate) fn undo(&mut self, document: &Document) -> Option<Restored> {
        let mut entry = self.undo.pop_back()?;
        let span = Span::new(entry.start, entry.start + entry.new.len());
        let edit = TextEdit::new(span, mem::take(&mut entry.old));
        let document = document.reparse(&edit);
        entry.old = edit.insert;
        let restored = Restored {
            document,
            changed: Span::new(entry.start, entry.start + entry.old.len()),
            selections: entry.before.clone(),
        };
        self.redo.push(entry);
        self.joinable = false;
        Some(restored)
    }

    pub(crate) fn redo(&mut self, document: &Document) -> Option<Restored> {
        let mut entry = self.redo.pop()?;
        let span = Span::new(entry.start, entry.start + entry.old.len());
        let edit = TextEdit::new(span, mem::take(&mut entry.new));
        let document = document.reparse(&edit);
        entry.new = edit.insert;
        let restored = Restored {
            document,
            changed: Span::new(entry.start, entry.start + entry.new.len()),
            selections: entry.after.clone(),
        };
        self.undo.push_back(entry);
        self.joinable = false;
        Some(restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::change::Change;
    use crate::selection::Selection;

    fn millis(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn type_at(buffer: &mut Buffer, text: &str, now: Duration) {
        let at = buffer.primary().head;
        let edit = TextEdit::new(Span::empty(at), text);
        buffer.apply(Change::new(vec![edit]), EditOrigin::Typing, now);
    }

    fn backspace(buffer: &mut Buffer, now: Duration) {
        let at = buffer.primary().head;
        let edit = TextEdit::new(Span::new(at - 1, at), "");
        buffer.apply(Change::new(vec![edit]), EditOrigin::Deletion, now);
    }

    #[test]
    fn a_typing_run_undoes_as_one_step() {
        let mut buffer = Buffer::new("");
        type_at(&mut buffer, "a", millis(0));
        type_at(&mut buffer, "b", millis(100));
        type_at(&mut buffer, "c", millis(450));
        assert_eq!(buffer.text(), "abc");
        assert_eq!(buffer.undo(), Some(Span::empty(0)));
        assert_eq!(buffer.text(), "");
        assert_eq!(buffer.selections(), [Selection::cursor(0)]);
        assert!(!buffer.can_undo());
        assert_eq!(buffer.redo(), Some(Span::new(0, 3)));
        assert_eq!(buffer.text(), "abc");
        assert_eq!(buffer.selections(), [Selection::cursor(3)]);
        assert_eq!(buffer.revision(), 5);
    }

    #[test]
    fn a_pause_of_the_group_delay_splits_the_run() {
        let mut buffer = Buffer::new("");
        type_at(&mut buffer, "a", millis(0));
        type_at(&mut buffer, "b", millis(500));
        buffer.undo();
        assert_eq!(buffer.text(), "a");
        assert_eq!(buffer.selections(), [Selection::cursor(1)]);
    }

    #[test]
    fn moving_the_caret_splits_the_run() {
        let mut buffer = Buffer::new("");
        type_at(&mut buffer, "ab", millis(0));
        buffer.select(Selection::cursor(0));
        type_at(&mut buffer, "c", millis(10));
        assert_eq!(buffer.text(), "cab");
        buffer.undo();
        assert_eq!(buffer.text(), "ab");
        assert_eq!(buffer.selections(), [Selection::cursor(0)]);
    }

    #[test]
    fn deletion_and_typing_are_separate_groups() {
        let mut buffer = Buffer::new("");
        type_at(&mut buffer, "abc", millis(0));
        backspace(&mut buffer, millis(10));
        backspace(&mut buffer, millis(20));
        type_at(&mut buffer, "x", millis(30));
        assert_eq!(buffer.text(), "ax");
        buffer.undo();
        assert_eq!(buffer.text(), "a");
        buffer.undo();
        assert_eq!(buffer.text(), "abc");
        assert_eq!(buffer.selections(), [Selection::cursor(3)]);
        buffer.undo();
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn paste_and_commands_never_merge() {
        let mut buffer = Buffer::new("");
        let paste = |at: usize| Change::new(vec![TextEdit::new(Span::empty(at), "p")]);
        buffer.apply(paste(0), EditOrigin::Paste, millis(0));
        buffer.apply(paste(1), EditOrigin::Paste, millis(1));
        buffer.undo();
        assert_eq!(buffer.text(), "p");
    }

    #[test]
    fn explicit_breaks_and_selection_changes_split_the_run() {
        let mut buffer = Buffer::new("");
        type_at(&mut buffer, "a", millis(0));
        buffer.break_undo_group();
        type_at(&mut buffer, "b", millis(1));
        let keep = Change::default().with_selections(vec![Selection::cursor(2)]);
        buffer.apply(keep, EditOrigin::Typing, millis(2));
        type_at(&mut buffer, "c", millis(3));
        buffer.undo();
        assert_eq!(buffer.text(), "ab");
        buffer.undo();
        assert_eq!(buffer.text(), "a");
    }

    #[test]
    fn a_new_edit_clears_redo_and_history_is_capped() {
        let mut buffer = Buffer::new("");
        for step in 0..=MAX_DEPTH {
            let at = buffer.text().len();
            let edit = TextEdit::new(Span::empty(at), "x");
            buffer.apply(Change::new(vec![edit]), EditOrigin::Command, millis(0));
            assert_eq!(buffer.text().len(), step + 1);
        }
        while buffer.undo().is_some() {}
        assert_eq!(buffer.text(), "x");
        assert!(buffer.can_redo());
        type_at(&mut buffer, "y", millis(0));
        assert!(!buffer.can_redo());
    }

    #[test]
    fn grouped_edits_around_each_other_compose() {
        let mut buffer = Buffer::new("hello");
        buffer.select(Selection::cursor(5));
        backspace(&mut buffer, millis(0));
        backspace(&mut buffer, millis(10));
        type_at(&mut buffer, "p", millis(20));
        assert_eq!(buffer.text(), "help");
        buffer.undo();
        assert_eq!(buffer.text(), "hel");
        assert_eq!(buffer.undo(), Some(Span::new(3, 5)));
        assert_eq!(buffer.text(), "hello");
        assert_eq!(buffer.selections(), [Selection::cursor(5)]);
        assert_eq!(buffer.redo(), Some(Span::empty(3)));
        assert_eq!(buffer.text(), "hel");
    }

    #[test]
    fn multi_cursor_typing_groups_and_undoes_in_one_step() {
        let mut buffer = Buffer::new("a\nb");
        buffer.set_selections(vec![Selection::cursor(1), Selection::cursor(3)], 0);
        for (index, letter) in ["x", "y"].into_iter().enumerate() {
            let edits = buffer
                .selections()
                .iter()
                .map(|selection| TextEdit::new(Span::empty(selection.head), letter))
                .collect();
            let now = millis(u64::try_from(index).expect("small index") * 10);
            buffer.apply(Change::new(edits), EditOrigin::Typing, now);
        }
        assert_eq!(buffer.text(), "axy\nbxy");
        assert_eq!(buffer.undo(), Some(Span::new(1, 3)));
        assert_eq!(buffer.text(), "a\nb");
        assert_eq!(
            buffer.selections(),
            [Selection::cursor(1), Selection::cursor(3)]
        );
    }
}
