use std::ops::Range;

const EDGE_SCROLL_MAX: f32 = 40.0;

#[must_use]
pub(crate) fn edge_scroll(y: f32, top: f32, bottom: f32) -> f32 {
    if y < top {
        -(top - y).min(EDGE_SCROLL_MAX)
    } else if y > bottom {
        (y - bottom).min(EDGE_SCROLL_MAX)
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Caret {
    pub page: usize,
    pub at: usize,
}

#[derive(Debug, Default)]
pub(crate) struct PdfSelection {
    anchor: Option<Caret>,
    head: Option<Caret>,
    dragging: bool,
    hovering_text: bool,
    pointer: Option<(f32, f32)>,
    autoscrolling: bool,
}

impl PdfSelection {
    pub(crate) const fn begin(&mut self, caret: Caret) {
        self.anchor = Some(caret);
        self.head = Some(caret);
        self.dragging = true;
    }

    pub(crate) const fn select(&mut self, page: usize, range: Range<usize>) {
        self.anchor = Some(Caret {
            page,
            at: range.start,
        });
        self.head = Some(Caret {
            page,
            at: range.end,
        });
        self.dragging = false;
    }

    pub(crate) fn extend(&mut self, caret: Caret) -> bool {
        if !self.dragging || self.head == Some(caret) {
            return false;
        }
        self.head = Some(caret);
        true
    }

    pub(crate) const fn select_all(&mut self, pages: usize) {
        if pages == 0 {
            return;
        }
        self.anchor = Some(Caret { page: 0, at: 0 });
        self.head = Some(Caret {
            page: pages - 1,
            at: usize::MAX,
        });
        self.dragging = false;
    }

    pub(crate) const fn pointer(&self) -> Option<(f32, f32)> {
        self.pointer
    }

    pub(crate) const fn set_pointer(&mut self, pointer: (f32, f32)) {
        self.pointer = Some(pointer);
    }

    pub(crate) fn start_autoscroll(&mut self) -> bool {
        let start = self.dragging && !self.autoscrolling;
        self.autoscrolling |= start;
        start
    }

    pub(crate) const fn stop_autoscroll(&mut self) {
        self.autoscrolling = false;
    }

    pub(crate) fn end(&mut self) -> bool {
        let was_dragging = self.dragging;
        self.dragging = false;
        self.autoscrolling = false;
        self.pointer = None;
        if self.anchor == self.head {
            self.anchor = None;
            self.head = None;
        }
        was_dragging
    }

    pub(crate) const fn clear(&mut self) {
        self.anchor = None;
        self.head = None;
        self.dragging = false;
    }

    pub(crate) const fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub(crate) const fn is_hovering_text(&self) -> bool {
        self.hovering_text
    }

    pub(crate) fn set_hovering_text(&mut self, hovering: bool) -> bool {
        let changed = self.hovering_text != hovering;
        self.hovering_text = hovering;
        changed
    }

    pub(crate) fn range(&self) -> Option<(Caret, Caret)> {
        let (anchor, head) = (self.anchor?, self.head?);
        (anchor != head).then(|| (anchor.min(head), anchor.max(head)))
    }

    pub(crate) fn on_page(&self, page: usize, len: usize) -> Option<Range<usize>> {
        let (start, end) = self.range()?;
        if page < start.page || page > end.page {
            return None;
        }
        let from = if page == start.page { start.at } else { 0 };
        let to = if page == end.page {
            end.at.min(len)
        } else {
            len
        };
        (from < to).then_some(from..to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn caret(page: usize, at: usize) -> Caret {
        Caret { page, at }
    }

    #[test]
    fn a_drag_selects_across_pages_in_document_order() {
        let mut selection = PdfSelection::default();
        selection.begin(caret(2, 5));
        assert!(selection.range().is_none());
        assert!(selection.extend(caret(0, 3)));
        assert!(!selection.extend(caret(0, 3)));
        assert!(selection.end());
        assert_eq!(selection.range(), Some((caret(0, 3), caret(2, 5))));
        assert_eq!(selection.on_page(0, 10), Some(3..10));
        assert_eq!(selection.on_page(1, 7), Some(0..7));
        assert_eq!(selection.on_page(2, 9), Some(0..5));
        assert_eq!(selection.on_page(3, 9), None);
        assert!(!selection.extend(caret(4, 0)));
    }

    #[test]
    fn a_click_without_drag_leaves_nothing_selected() {
        let mut selection = PdfSelection::default();
        selection.begin(caret(1, 4));
        selection.end();
        assert!(selection.range().is_none());
        selection.select(1, 2..6);
        assert_eq!(selection.on_page(1, 10), Some(2..6));
        selection.clear();
        assert!(selection.range().is_none());
    }

    #[test]
    fn select_all_covers_every_page_to_the_end() {
        let mut selection = PdfSelection::default();
        selection.select_all(0);
        assert!(selection.range().is_none());
        selection.select_all(3);
        assert_eq!(selection.on_page(0, 12), Some(0..12));
        assert_eq!(selection.on_page(1, 5), Some(0..5));
        assert_eq!(selection.on_page(2, 7), Some(0..7));
    }

    #[test]
    fn dragging_past_an_edge_scrolls_until_the_drag_ends() {
        assert!(edge_scroll(50.0, 100.0, 500.0) < 0.0);
        assert!(edge_scroll(520.0, 100.0, 500.0) > 0.0);
        assert!(edge_scroll(300.0, 100.0, 500.0).abs() < f32::EPSILON);
        assert!((edge_scroll(9_000.0, 100.0, 500.0) - EDGE_SCROLL_MAX).abs() < f32::EPSILON);
        let mut selection = PdfSelection::default();
        assert!(!selection.start_autoscroll());
        selection.begin(caret(0, 1));
        selection.set_pointer((10.0, 600.0));
        assert!(selection.start_autoscroll());
        assert!(!selection.start_autoscroll());
        selection.end();
        assert!(selection.pointer().is_none() && !selection.start_autoscroll());
    }
}
