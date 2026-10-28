use std::collections::BTreeSet;

use margin_files::{SortBy, TreeRow};
use margin_kernel::RelativePath;

use crate::paths;
use crate::selection::{SelectedEntry, Selection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gesture {
    Plain { click_count: usize },
    Toggle,
    Extend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Key {
    Next,
    Previous,
    Expand,
    Collapse,
    Activate,
    Rename,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    None,
    Redraw,
    Rebuild,
    Open(RelativePath),
    Reveal(RelativePath),
    Rename(RelativePath),
    Delete(SelectedEntry),
}

pub(crate) enum Item<'rows> {
    Row(&'rows TreeRow),
    NewFolder { depth: usize },
}

#[derive(Debug)]
pub(crate) struct TreeModel {
    rows: Vec<TreeRow>,
    expanded: BTreeSet<RelativePath>,
    selection: Selection,
    selected_folder: Option<RelativePath>,
    focus: Option<RelativePath>,
    sort_by: SortBy,
    pending_new_folder: Option<RelativePath>,
    reveal: Option<RelativePath>,
    active: Option<RelativePath>,
}

impl TreeModel {
    pub(crate) fn new() -> Self {
        Self {
            rows: Vec::new(),
            expanded: BTreeSet::new(),
            selection: Selection::default(),
            selected_folder: None,
            focus: None,
            sort_by: SortBy::Name,
            pending_new_folder: None,
            reveal: None,
            active: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    pub(crate) fn row(&self, path: &RelativePath) -> Option<&TreeRow> {
        self.rows.iter().find(|row| &row.path == path)
    }

    pub(crate) fn set_rows(&mut self, rows: Vec<TreeRow>) {
        self.rows = rows;
        let live: BTreeSet<&RelativePath> = self.rows.iter().map(|row| &row.path).collect();
        let hidden_by_collapse = |path: &RelativePath| {
            paths::ancestors(path)
                .iter()
                .any(|folder| live.contains(folder) && !self.expanded.contains(folder))
        };
        self.selection
            .prune(|path| live.contains(path) || hidden_by_collapse(path));
    }

    pub(crate) fn expanded(&self) -> &BTreeSet<RelativePath> {
        &self.expanded
    }

    pub(crate) fn is_expanded(&self, path: &RelativePath) -> bool {
        self.expanded.contains(path)
    }

    pub(crate) fn set_expanded(&mut self, folders: Vec<RelativePath>) {
        self.expanded = folders.into_iter().collect();
    }

    pub(crate) fn expand(&mut self, folder: &RelativePath) -> bool {
        !folder.is_root() && self.expanded.insert(folder.clone())
    }

    pub(crate) fn expand_ancestors(&mut self, path: &RelativePath) -> bool {
        let mut changed = false;
        for folder in paths::ancestors(path) {
            changed |= self.expanded.insert(folder);
        }
        changed
    }

    pub(crate) fn collapse(&mut self, folder: &RelativePath) {
        self.expanded.remove(folder);
        let Some(index) = self.rows.iter().position(|row| &row.path == folder) else {
            return;
        };
        let depth = self.rows[index].depth;
        let end = self.rows[index + 1..]
            .iter()
            .position(|row| row.depth <= depth)
            .map_or(self.rows.len(), |offset| index + 1 + offset);
        self.rows.drain(index + 1..end);
    }

    pub(crate) fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    pub(crate) fn toggle_folder(&mut self, folder: &RelativePath) -> Effect {
        if self.expanded.contains(folder) {
            self.collapse(folder);
            Effect::Redraw
        } else {
            self.expanded.insert(folder.clone());
            Effect::Rebuild
        }
    }

    pub(crate) const fn sort_by(&self) -> SortBy {
        self.sort_by
    }

    pub(crate) const fn set_sort_by(&mut self, sort_by: SortBy) {
        self.sort_by = sort_by;
    }

    pub(crate) const fn selection(&self) -> &Selection {
        &self.selection
    }

    pub(crate) fn select(&mut self, path: &RelativePath, is_dir: bool) {
        self.selection.select_single(path, is_dir);
        self.selected_folder = is_dir.then(|| path.clone());
        self.focus = Some(path.clone());
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selection.clear();
        self.selected_folder = Some(RelativePath::root());
    }

    pub(crate) fn select_all(&mut self) {
        self.selection.select_all(&self.rows);
    }

    pub(crate) const fn active(&self) -> Option<&RelativePath> {
        self.active.as_ref()
    }

    pub(crate) fn set_active(&mut self, path: Option<RelativePath>) -> bool {
        if self.active == path {
            return false;
        }
        if let Some(path) = &path
            && !self.selection.contains(path)
        {
            self.selection.select_single(path, false);
        }
        self.active = path;
        true
    }

    pub(crate) fn target_folder(&self) -> RelativePath {
        match self.selection.primary() {
            Some(entry) if entry.is_dir() => entry.path().clone(),
            Some(entry) => paths::parent(entry.path()),
            None => self
                .selected_folder
                .clone()
                .or_else(|| self.active.as_ref().map(paths::parent))
                .unwrap_or_default(),
        }
    }

    pub(crate) fn focus_target(&self) -> Option<&TreeRow> {
        self.focus
            .as_ref()
            .and_then(|focus| self.row(focus))
            .or_else(|| self.rows.first())
    }

    pub(crate) fn focused(&self) -> Option<&RelativePath> {
        self.focus_target().map(|row| &row.path)
    }

    pub(crate) fn set_focus(&mut self, path: &RelativePath) {
        self.focus = Some(path.clone());
    }

    pub(crate) fn set_pending_new_folder(&mut self, parent: Option<RelativePath>) {
        self.pending_new_folder = parent;
    }

    pub(crate) fn remap(&mut self, from: &RelativePath, to: &RelativePath) {
        self.expanded = self
            .expanded
            .iter()
            .map(|folder| paths::remap(folder, from, to))
            .collect();
        self.selection.remap(from, to);
        for path in [&mut self.selected_folder, &mut self.focus, &mut self.active]
            .into_iter()
            .flatten()
        {
            *path = paths::remap(path, from, to);
        }
    }

    pub(crate) fn forget(&mut self, removed: &RelativePath) {
        if self
            .selected_folder
            .as_ref()
            .is_some_and(|folder| folder.starts_with(removed))
        {
            self.selected_folder = Some(RelativePath::root());
        }
    }

    pub(crate) fn item_count(&self) -> usize {
        self.rows.len() + usize::from(self.pending_new_folder.is_some())
    }

    fn insert_index(&self) -> Option<usize> {
        let parent = self.pending_new_folder.as_ref()?;
        let Some(index) = self.rows.iter().position(|row| &row.path == parent) else {
            return Some(0);
        };
        let depth = self.rows[index].depth;
        Some(
            self.rows[index + 1..]
                .iter()
                .position(|row| row.depth <= depth)
                .map_or(self.rows.len(), |offset| index + 1 + offset),
        )
    }

    pub(crate) fn item(&self, index: usize) -> Option<Item<'_>> {
        match self.insert_index() {
            Some(insert) if index == insert => {
                let parent = self.pending_new_folder.as_ref()?;
                let depth = self.row(parent).map_or(0, |row| row.depth + 1);
                Some(Item::NewFolder { depth })
            }
            Some(insert) if index > insert => self.rows.get(index - 1).map(Item::Row),
            _ => self.rows.get(index).map(Item::Row),
        }
    }

    pub(crate) fn item_index(&self, path: &RelativePath) -> Option<usize> {
        let index = self.rows.iter().position(|row| &row.path == path)?;
        Some(match self.insert_index() {
            Some(insert) if index >= insert => index + 1,
            _ => index,
        })
    }

    pub(crate) fn request_reveal(&mut self, path: &RelativePath) {
        self.reveal = Some(path.clone());
    }

    pub(crate) fn take_scroll_target(&mut self) -> Option<usize> {
        let index = match &self.reveal {
            Some(path) => self.item_index(path),
            None => self.insert_index(),
        };
        if index.is_some() {
            self.reveal = None;
        }
        index
    }

    pub(crate) fn click(&mut self, path: &RelativePath, gesture: Gesture) -> Effect {
        let Some(row) = self.row(path) else {
            return Effect::None;
        };
        let is_dir = row.is_dir;
        self.focus = Some(path.clone());
        match gesture {
            Gesture::Toggle => self.selection.toggle(path, is_dir),
            Gesture::Extend => self.selection.select_range(path, &self.rows),
            Gesture::Plain { click_count: 3.. } => return Effect::Rename(path.clone()),
            Gesture::Plain { click_count } => {
                self.select(path, is_dir);
                return match (is_dir, click_count) {
                    (true, 1) => self.toggle_folder(path),
                    (true, _) => Effect::Redraw,
                    (false, _) => Effect::Open(path.clone()),
                };
            }
        }
        Effect::Redraw
    }

    pub(crate) fn press(&mut self, key: Key) -> Effect {
        let Some(index) = self
            .focus_target()
            .and_then(|row| self.rows.iter().position(|candidate| candidate == row))
        else {
            return Effect::None;
        };
        let row = self.rows[index].clone();
        let next = self.rows.get(index + 1).cloned();
        let previous = index
            .checked_sub(1)
            .map(|previous| self.rows[previous].path.clone());
        match key {
            Key::Next => next.map_or(Effect::None, |next| self.move_focus(&next.path)),
            Key::Previous => previous.map_or(Effect::None, |previous| self.move_focus(&previous)),
            Key::Expand if row.is_dir && !self.is_expanded(&row.path) => {
                self.expanded.insert(row.path);
                Effect::Rebuild
            }
            Key::Expand => match next {
                Some(child) if row.is_dir && child.depth > row.depth => {
                    self.move_focus(&child.path)
                }
                _ => Effect::None,
            },
            Key::Collapse if row.is_dir && self.is_expanded(&row.path) => {
                self.collapse(&row.path);
                Effect::Redraw
            }
            Key::Collapse => {
                let parent = paths::parent(&row.path);
                if self.row(&parent).is_some() {
                    self.move_focus(&parent)
                } else {
                    Effect::None
                }
            }
            Key::Activate => self.click(&row.path, Gesture::Plain { click_count: 1 }),
            Key::Rename => Effect::Rename(row.path),
            Key::Delete => Effect::Delete(SelectedEntry::new(row.path, row.is_dir)),
        }
    }

    fn move_focus(&mut self, path: &RelativePath) -> Effect {
        self.focus = Some(path.clone());
        Effect::Reveal(path.clone())
    }
}
