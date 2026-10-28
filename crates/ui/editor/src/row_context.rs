use std::rc::Rc;

use gpui_kit::{App, Entity, RetainAllImageCache, Window};
use margin_kernel::RelativePath;
use margin_live_preview::{CalloutFolds, Mode, Reveal};
use margin_markdown::{Fold, Span};
use margin_table_editing::TableOp;
use margin_theme::Palette;

use crate::hit_map::HitMap;
use crate::segment::Overlay;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RowAction {
    ToggleTask { marker: Span },
    ToggleFold { block_start: usize, fold: Fold },
    CopyCode { content: Span },
    OpenImage { source: Span },
    PlaceCaret(usize),
    OpenPath(RelativePath),
    JumpToFootnote(String),
    Table { at: usize, op: TableOp },
}

pub(crate) type RowHandler = Rc<dyn Fn(RowAction, &mut Window, &mut App)>;

#[derive(Clone)]
pub(crate) struct Surface {
    palette: Palette,
    mode: Mode,
    overlay: Rc<Overlay>,
    hits: HitMap,
    images: Entity<RetainAllImageCache>,
}

impl Surface {
    pub(crate) const fn new(
        palette: Palette,
        mode: Mode,
        overlay: Rc<Overlay>,
        hits: HitMap,
        images: Entity<RetainAllImageCache>,
    ) -> Self {
        Self {
            palette,
            mode,
            overlay,
            hits,
            images,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RowContext {
    surface: Surface,
    handler: RowHandler,
    note: RelativePath,
    folds: Rc<CalloutFolds>,
    reveal: Rc<Reveal>,
    depth: usize,
}

impl RowContext {
    pub(crate) fn new(
        surface: Surface,
        handler: RowHandler,
        note: RelativePath,
        folds: Rc<CalloutFolds>,
        reveal: Rc<Reveal>,
    ) -> Self {
        Self {
            surface,
            handler,
            note,
            folds,
            reveal,
            depth: 0,
        }
    }

    pub(crate) fn embedded(&self, note: RelativePath, overlay: Rc<Overlay>) -> Self {
        let parent = Rc::clone(&self.handler);
        let handler: RowHandler = Rc::new(move |action, window, cx| {
            if matches!(action, RowAction::OpenPath(_)) {
                parent(action, window, cx);
            }
        });
        Self {
            surface: Surface {
                palette: self.surface.palette.clone(),
                mode: Mode::LivePreview,
                overlay,
                hits: HitMap::default(),
                images: self.surface.images.clone(),
            },
            handler,
            note,
            folds: Rc::new(CalloutFolds::default()),
            reveal: Rc::new(Reveal::default()),
            depth: self.depth + 1,
        }
    }

    pub(crate) const fn palette(&self) -> &Palette {
        &self.surface.palette
    }

    pub(crate) const fn mode(&self) -> Mode {
        self.surface.mode
    }

    pub(crate) fn overlay(&self) -> Rc<Overlay> {
        Rc::clone(&self.surface.overlay)
    }

    pub(crate) fn hits(&self) -> HitMap {
        self.surface.hits.clone()
    }

    pub(crate) const fn images(&self) -> &Entity<RetainAllImageCache> {
        &self.surface.images
    }

    pub(crate) fn handler(&self) -> RowHandler {
        Rc::clone(&self.handler)
    }

    pub(crate) const fn note(&self) -> &RelativePath {
        &self.note
    }

    pub(crate) fn is_collapsed(&self, block_start: usize, fold: Fold) -> bool {
        self.folds.is_collapsed(block_start, fold)
    }

    pub(crate) fn reveal(&self) -> &Reveal {
        &self.reveal
    }

    pub(crate) const fn depth(&self) -> usize {
        self.depth
    }

    pub(crate) const fn is_embedded(&self) -> bool {
        self.depth > 0
    }
}
