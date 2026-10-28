use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::{Bounds, Pixels, Point, TextLayout, point, px, size};
use margin_live_preview::{LinkHit, OffsetMap};

#[derive(Clone)]
pub(crate) struct Segment {
    layout: TextLayout,
    map: Rc<OffsetMap>,
    display: Range<usize>,
    bounds: Bounds<Pixels>,
    links: Rc<[LinkHit]>,
}

impl Segment {
    pub(crate) const fn new(
        layout: TextLayout,
        map: Rc<OffsetMap>,
        display: Range<usize>,
        bounds: Bounds<Pixels>,
        links: Rc<[LinkHit]>,
    ) -> Self {
        Self {
            layout,
            map,
            display,
            bounds,
            links,
        }
    }

    fn link_at(&self, position: Point<Pixels>) -> Option<LinkHit> {
        if !self.bounds.contains(&position) {
            return None;
        }
        let Ok(index) = self.layout.index_for_position(position) else {
            return None;
        };
        let display = self.display.start + index;
        self.links
            .iter()
            .find(|link| link.display.start <= display && display < link.display.end)
            .cloned()
    }

    fn source_at(&self, position: Point<Pixels>) -> usize {
        let index = match self.layout.index_for_position(position) {
            Ok(index) | Err(index) => index,
        };
        let display = (self.display.start + index).min(self.display.end);
        self.map.to_source(display)
    }

    fn covers(&self, offset: usize) -> bool {
        if offset < self.map.source_start() || offset > self.map.source_end() {
            return false;
        }
        self.display.contains(&self.map.to_display(offset))
            || self.display.end == self.map.to_display(offset)
    }

    fn caret_bounds(&self, offset: usize) -> Bounds<Pixels> {
        let display = self.map.to_display(offset);
        let index = display.saturating_sub(self.display.start);
        let line_height = self.layout.line_height();
        let origin = self
            .layout
            .position_for_index(index)
            .unwrap_or(self.bounds.origin);
        Bounds::new(origin, size(Pixels::ZERO, line_height))
    }

    fn distance(&self, position: Point<Pixels>) -> (Pixels, Pixels) {
        let bounds = self.bounds;
        let vertical = if position.y < bounds.top() {
            bounds.top() - position.y
        } else if position.y > bounds.bottom() {
            position.y - bounds.bottom()
        } else {
            Pixels::ZERO
        };
        let horizontal = if position.x < bounds.left() {
            bounds.left() - position.x
        } else if position.x > bounds.right() {
            position.x - bounds.right()
        } else {
            Pixels::ZERO
        };
        (vertical, horizontal)
    }
}

#[derive(Clone, Default)]
pub(crate) struct HitMap(Rc<RefCell<Vec<Segment>>>);

impl HitMap {
    pub(crate) fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    pub(crate) fn push(&self, segment: Segment) {
        self.0.borrow_mut().push(segment);
    }

    pub(crate) fn offset_at(&self, position: Point<Pixels>) -> Option<usize> {
        let segments = self.0.borrow();
        let nearest = segments
            .iter()
            .min_by_key(|segment| segment.distance(position))?;
        let bounds = nearest.bounds;
        let clamped = point(
            position.x.clamp(bounds.left(), bounds.right()),
            position.y.clamp(bounds.top(), bounds.bottom() - px(1.0)),
        );
        Some(nearest.source_at(clamped))
    }

    pub(crate) fn link_at(&self, position: Point<Pixels>) -> Option<LinkHit> {
        self.0
            .borrow()
            .iter()
            .find_map(|segment| segment.link_at(position))
    }

    pub(crate) fn caret_bounds(&self, offset: usize) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .iter()
            .find(|segment| segment.covers(offset))
            .map(|segment| segment.caret_bounds(offset))
    }

    pub(crate) fn vertical_neighbor(&self, offset: usize, x: Pixels, below: bool) -> Option<usize> {
        let caret = self.caret_bounds(offset)?;
        let line_height = caret.size.height;
        let y = if below {
            caret.bottom() + line_height / 2.0
        } else {
            caret.top() - line_height / 2.0
        };
        let segments = self.0.borrow();
        let reachable = segments.iter().any(|segment| {
            if below {
                segment.bounds.bottom() > y
            } else {
                segment.bounds.top() < y
            }
        });
        drop(segments);
        reachable.then(|| self.offset_at(point(x, y))).flatten()
    }
}
