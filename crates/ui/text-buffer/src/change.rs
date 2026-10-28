use margin_markdown::TextEdit;

use crate::selection::Selection;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub edits: Vec<TextEdit>,
    pub selections: Option<Vec<Selection>>,
}

impl Change {
    #[must_use]
    pub const fn new(edits: Vec<TextEdit>) -> Self {
        Self {
            edits,
            selections: None,
        }
    }

    #[must_use]
    pub fn with_selections(mut self, selections: Vec<Selection>) -> Self {
        self.selections = Some(selections);
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty() && self.selections.is_none()
    }
}
