use gpui_kit::{ListAlignment, ListState, px};
use margin_files::FsEntry;
use margin_search::{SearchHit, TagInfo};

use crate::items::{self, Item, Row};
use crate::query::Query;

const LIST_OVERDRAW: f32 = 240.0;

#[derive(Debug)]
pub(crate) struct Results {
    query: Query,
    selected_tag: Option<String>,
    tags: Vec<TagInfo>,
    tags_loading: bool,
    names: Vec<FsEntry>,
    hits: Vec<SearchHit>,
    searching: bool,
    items: Vec<Item>,
    rows: Vec<Row>,
    selected: usize,
    list: ListState,
}

impl Default for Results {
    fn default() -> Self {
        Self {
            query: Query::Empty,
            selected_tag: None,
            tags: Vec::new(),
            tags_loading: false,
            names: Vec::new(),
            hits: Vec::new(),
            searching: false,
            items: Vec::new(),
            rows: Vec::new(),
            selected: 0,
            list: ListState::new(0, ListAlignment::Top, px(LIST_OVERDRAW)),
        }
    }
}

impl Results {
    pub(crate) const fn query(&self) -> &Query {
        &self.query
    }

    pub(crate) fn selected_tag(&self) -> Option<&str> {
        self.selected_tag.as_deref()
    }

    pub(crate) const fn tags_loading(&self) -> bool {
        self.tags_loading
    }

    pub(crate) const fn is_searching(&self) -> bool {
        self.searching
    }

    pub(crate) fn hits(&self) -> &[SearchHit] {
        &self.hits
    }

    pub(crate) fn items(&self) -> &[Item] {
        &self.items
    }

    pub(crate) fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub(crate) const fn selected(&self) -> usize {
        self.selected
    }

    pub(crate) fn list(&self) -> ListState {
        self.list.clone()
    }

    pub(crate) fn can_replace(&self) -> bool {
        self.query.search_text().is_some() && !self.hits.is_empty()
    }

    pub(crate) fn set_query(&mut self, query: Query, selected_tag: Option<String>) {
        self.query = query;
        self.selected_tag = selected_tag;
        if self.query.search_text().is_none() {
            self.names.clear();
            self.hits.clear();
            self.searching = false;
        }
        self.rebuild();
    }

    pub(crate) fn set_searching(&mut self) {
        self.searching = true;
    }

    pub(crate) fn set_search_results(&mut self, names: Vec<FsEntry>, hits: Vec<SearchHit>) {
        self.names = names;
        self.hits = hits;
        self.searching = false;
        self.rebuild();
    }

    pub(crate) fn set_tags_loading(&mut self) {
        self.tags_loading = true;
    }

    pub(crate) fn set_tags(&mut self, tags: Vec<TagInfo>) {
        self.tags = tags;
        self.tags_loading = false;
        self.rebuild();
    }

    pub(crate) fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected = index;
        }
    }

    pub(crate) fn move_selection(&mut self, step: isize) {
        let Some(last) = self.items.len().checked_sub(1) else {
            return;
        };
        self.selected = self.selected.saturating_add_signed(step).min(last);
        if let Some(row) = self
            .rows
            .iter()
            .position(|row| *row == Row::Item(self.selected))
        {
            self.list.scroll_to_reveal_item(row);
        }
    }

    fn rebuild(&mut self) {
        self.items = items::build_items(
            &self.query,
            self.selected_tag.as_deref(),
            &self.tags,
            &self.names,
            &self.hits,
        );
        self.rows = items::rows(&self.items);
        self.selected = 0;
        self.list.reset(self.rows.len());
    }
}

#[cfg(test)]
mod tests {
    use margin_kernel::{RelativePath, UnixMillis};

    use super::*;

    fn entry(raw: &str) -> FsEntry {
        FsEntry {
            name: raw.to_owned(),
            is_dir: false,
            path: RelativePath::new(raw).expect("valid test path"),
            modified: UnixMillis::default(),
        }
    }

    #[test]
    fn selection_stays_within_the_items() {
        let mut results = Results::default();
        results.set_query(Query::Search("a".to_owned()), None);
        results.set_search_results(vec![entry("a.md"), entry("b.md")], Vec::new());
        results.move_selection(-1);
        assert_eq!(results.selected(), 0);
        results.move_selection(1);
        results.move_selection(1);
        assert_eq!(results.selected(), 1);
    }

    #[test]
    fn leaving_search_drops_its_results() {
        let mut results = Results::default();
        results.set_query(Query::Search("a".to_owned()), None);
        results.set_search_results(vec![entry("a.md")], Vec::new());
        results.set_query(Query::Ask("a".to_owned()), None);
        assert!(results.items().is_empty());
        results.set_query(Query::Search("a".to_owned()), None);
        assert!(results.items().is_empty());
    }
}
