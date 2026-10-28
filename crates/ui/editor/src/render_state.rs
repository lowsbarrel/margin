use std::cell::RefCell;

use gpui_kit::{App, Entity, RetainAllImageCache};

use crate::caret_blink::CaretBlink;
use crate::table_virtual::TableCache;

pub(crate) struct RenderState {
    tables: RefCell<TableCache>,
    images: Entity<RetainAllImageCache>,
    caret: CaretBlink,
}

impl RenderState {
    pub(crate) fn new(cx: &mut App) -> Self {
        Self {
            tables: RefCell::default(),
            images: RetainAllImageCache::new(cx),
            caret: CaretBlink::default(),
        }
    }

    pub(crate) const fn tables(&self) -> &RefCell<TableCache> {
        &self.tables
    }

    pub(crate) const fn images(&self) -> &Entity<RetainAllImageCache> {
        &self.images
    }

    pub(crate) const fn caret(&self) -> &CaretBlink {
        &self.caret
    }

    pub(crate) const fn caret_mut(&mut self) -> &mut CaretBlink {
        &mut self.caret
    }
}
