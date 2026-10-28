use std::mem;
use std::rc::Rc;

use crate::model::Drawing;

const MAX_STEPS: usize = 200;

#[derive(Debug, Default)]
pub(crate) struct UndoHistory {
    undo: Vec<Rc<Drawing>>,
    redo: Vec<Rc<Drawing>>,
}

impl UndoHistory {
    pub(crate) fn record(&mut self, before: &Rc<Drawing>) {
        if self.undo.len() == MAX_STEPS {
            self.undo.remove(0);
        }
        self.undo.push(Rc::clone(before));
        self.redo.clear();
    }

    pub(crate) fn undo(&mut self, current: &mut Rc<Drawing>) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(mem::replace(current, previous));
        true
    }

    pub(crate) fn redo(&mut self, current: &mut Rc<Drawing>) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(mem::replace(current, next));
        true
    }

    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{INK_COLOR, TextLabel};

    #[test]
    fn undo_and_redo_walk_snapshots_and_new_edits_drop_redo() {
        let mut history = UndoHistory::default();
        let mut drawing = Rc::new(Drawing::default());
        history.record(&drawing);
        Rc::make_mut(&mut drawing).labels.push(TextLabel {
            x: 0.0,
            y: 0.0,
            text: "a".to_owned(),
            color: INK_COLOR.to_owned(),
            font_size: 16.0,
        });
        assert!(history.undo(&mut drawing));
        assert!(drawing.is_empty());
        assert!(history.redo(&mut drawing));
        assert_eq!(drawing.labels.len(), 1);
        assert!(history.undo(&mut drawing));
        history.record(&drawing);
        assert!(!history.redo(&mut drawing));
    }
}
