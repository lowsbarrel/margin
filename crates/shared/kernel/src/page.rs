use serde::{Deserialize, Serialize};

pub const DEFAULT_PAGE_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cursor(String);

impl Cursor {
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn offset(&self) -> Option<usize> {
        self.0.parse().ok()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRequest {
    pub cursor: Option<Cursor>,
    pub limit: usize,
}

impl PageRequest {
    #[must_use]
    pub const fn first(limit: usize) -> Self {
        Self {
            cursor: None,
            limit,
        }
    }

    #[must_use]
    pub const fn after(cursor: Cursor, limit: usize) -> Self {
        Self {
            cursor: Some(cursor),
            limit,
        }
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self::first(DEFAULT_PAGE_LIMIT)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<Cursor>,
}

impl<T> Page<T> {
    #[must_use]
    pub fn from_ordered(items: Vec<T>, request: &PageRequest) -> Self {
        let start = request
            .cursor
            .as_ref()
            .and_then(Cursor::offset)
            .unwrap_or(0)
            .min(items.len());
        let limit = request.limit.max(1);
        let end = start.saturating_add(limit).min(items.len());
        let next = (end < items.len()).then(|| Cursor(end.to_string()));
        let items = items.into_iter().skip(start).take(end - start).collect();
        Self { items, next }
    }

    #[must_use]
    pub fn is_last(&self) -> bool {
        self.next.is_none()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_an_ordered_list_page_by_page() {
        let numbers: Vec<u32> = (0..5).collect();
        let first = Page::from_ordered(numbers.clone(), &PageRequest::first(2));
        assert_eq!(first.items, [0, 1]);
        let cursor = first.next.expect("more items follow");
        let second = Page::from_ordered(numbers.clone(), &PageRequest::after(cursor, 2));
        assert_eq!(second.items, [2, 3]);
        let last = Page::from_ordered(
            numbers,
            &PageRequest::after(second.next.expect("one item left"), 2),
        );
        assert_eq!(last.items, [4]);
        assert!(last.is_last());
    }

    #[test]
    fn tolerates_stale_or_foreign_cursors() {
        let numbers: Vec<u32> = (0..3).collect();
        let past_end = Page::from_ordered(
            numbers.clone(),
            &PageRequest::after(Cursor::new(String::from("9")), 2),
        );
        assert!(past_end.is_empty() && past_end.is_last());
        let garbage = Page::from_ordered(
            numbers,
            &PageRequest::after(Cursor::new(String::from("x")), 0),
        );
        assert_eq!(garbage.items, [0]);
    }
}
