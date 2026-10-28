use margin_files::TreeRow;
use margin_kernel::RelativePath;

use crate::paths;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectedEntry {
    path: RelativePath,
    is_dir: bool,
}

impl SelectedEntry {
    pub(crate) const fn new(path: RelativePath, is_dir: bool) -> Self {
        Self { path, is_dir }
    }

    pub(crate) const fn path(&self) -> &RelativePath {
        &self.path
    }

    pub(crate) const fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub(crate) fn into_path(self) -> RelativePath {
        self.path
    }

    fn of(row: &TreeRow) -> Self {
        Self::new(row.path.clone(), row.is_dir)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Selection {
    entries: Vec<SelectedEntry>,
    last: Option<RelativePath>,
}

impl Selection {
    pub(crate) fn entries(&self) -> &[SelectedEntry] {
        &self.entries
    }

    pub(crate) fn contains(&self, path: &RelativePath) -> bool {
        self.entries.iter().any(|entry| &entry.path == path)
    }

    pub(crate) fn targets(&self, entry: SelectedEntry) -> Vec<SelectedEntry> {
        if self.entries.len() > 1 && self.contains(entry.path()) {
            self.entries.clone()
        } else {
            vec![entry]
        }
    }

    pub(crate) fn drag_targets(&self, grabbed: &SelectedEntry) -> Vec<SelectedEntry> {
        let mut targets = self.targets(grabbed.clone());
        if let Some(index) = targets.iter().position(|entry| entry.path == grabbed.path) {
            targets[..=index].rotate_right(1);
        }
        targets
    }

    pub(crate) fn clipboard_targets(&self, entry: Option<SelectedEntry>) -> Vec<SelectedEntry> {
        match entry {
            Some(entry) if self.entries.len() <= 1 => vec![entry],
            _ => self.entries.clone(),
        }
    }

    pub(crate) fn primary(&self) -> Option<&SelectedEntry> {
        if let [only] = self.entries.as_slice() {
            return Some(only);
        }
        let last = self.last.as_ref()?;
        self.entries.iter().find(|entry| &entry.path == last)
    }

    pub(crate) fn select_single(&mut self, path: &RelativePath, is_dir: bool) {
        self.entries = vec![SelectedEntry::new(path.clone(), is_dir)];
        self.last = Some(path.clone());
    }

    pub(crate) fn toggle(&mut self, path: &RelativePath, is_dir: bool) {
        if self.contains(path) {
            self.entries.retain(|entry| &entry.path != path);
        } else {
            self.entries.push(SelectedEntry::new(path.clone(), is_dir));
        }
        self.last = Some(path.clone());
    }

    pub(crate) fn select_range(&mut self, path: &RelativePath, rows: &[TreeRow]) {
        let position = |target: &RelativePath| rows.iter().position(|row| &row.path == target);
        let Some(anchor) = self.last.clone() else {
            if let Some(index) = position(path) {
                self.select_single(path, rows[index].is_dir);
            }
            return;
        };
        let (Some(from), Some(to)) = (position(&anchor), position(path)) else {
            return;
        };
        let range = from.min(to)..=from.max(to);
        self.entries = rows[range].iter().map(SelectedEntry::of).collect();
    }

    pub(crate) fn select_all(&mut self, rows: &[TreeRow]) {
        self.entries = rows.iter().map(SelectedEntry::of).collect();
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.last = None;
    }

    pub(crate) fn prune(&mut self, keep: impl Fn(&RelativePath) -> bool) {
        self.entries.retain(|entry| keep(&entry.path));
        if self.last.as_ref().is_some_and(|last| !keep(last)) {
            self.last = None;
        }
    }

    pub(crate) fn remap(&mut self, from: &RelativePath, to: &RelativePath) {
        for entry in &mut self.entries {
            entry.path = paths::remap(&entry.path, from, to);
        }
        if let Some(last) = &mut self.last {
            *last = paths::remap(last, from, to);
        }
    }
}

#[cfg(test)]
mod tests {
    use margin_kernel::UnixMillis;

    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("valid path")
    }

    fn rows(paths: &[&str]) -> Vec<TreeRow> {
        paths
            .iter()
            .map(|raw| TreeRow {
                name: raw.to_owned().to_owned(),
                path: path(raw),
                is_dir: false,
                modified: UnixMillis::default(),
                depth: 0,
            })
            .collect()
    }

    fn selected(selection: &Selection) -> Vec<&str> {
        selection
            .entries()
            .iter()
            .map(|entry| entry.path().as_str())
            .collect()
    }

    #[test]
    fn shift_selects_the_range_from_the_last_click() {
        let rows = rows(&["a", "b", "c", "d"]);
        let mut selection = Selection::default();
        selection.select_single(&path("b"), false);
        selection.select_range(&path("d"), &rows);
        assert_eq!(selected(&selection), ["b", "c", "d"]);
        selection.select_range(&path("a"), &rows);
        assert_eq!(selected(&selection), ["a", "b"]);
    }

    #[test]
    fn toggling_adds_and_removes_and_moves_the_anchor() {
        let mut selection = Selection::default();
        selection.select_single(&path("a"), false);
        selection.toggle(&path("c"), false);
        assert_eq!(selected(&selection), ["a", "c"]);
        assert_eq!(
            selection.primary().map(SelectedEntry::path),
            Some(&path("c"))
        );
        selection.toggle(&path("a"), false);
        assert_eq!(selected(&selection), ["c"]);
    }

    #[test]
    fn renaming_a_folder_moves_selected_descendants() {
        let mut selection = Selection::default();
        selection.select_single(&path("notes/a.md"), false);
        selection.toggle(&path("notesy.md"), false);
        selection.remap(&path("notes"), &path("archive/notes"));
        assert_eq!(selected(&selection), ["archive/notes/a.md", "notesy.md"]);
    }
}
