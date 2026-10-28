use gpui_kit::{Context, Pixels, Point, Window};
use margin_workbench::DraggedFiles;

use crate::drag::{Dragged, DraggedTab, DropZone, strip_accepts, strip_drop_gap, strip_gap_at};
use crate::layout::Side;
use crate::panes::{PaneItem, Panes, PanesEvent};

const MIN_PANE_WIDTH: f32 = 120.0;

#[expect(
    clippy::multiple_inherent_impl,
    reason = "drag and drop handling lives beside the strip and zone renderers that trigger it"
)]
impl<T: PaneItem> Panes<T> {
    pub(crate) fn dragged_tab(&self, payload: &DraggedTab) -> Option<Dragged> {
        let (pane, tab) = self.position_of(payload.item)?;
        Some(Dragged::Tab {
            pane,
            tab,
            pinned: payload.pinned,
        })
    }

    pub(crate) fn drag_moved(
        &mut self,
        dragged: Option<Dragged>,
        position: Point<Pixels>,
        cx: &mut Context<'_, Self>,
    ) {
        let target = dragged.and_then(|dragged| self.strip_target_at(dragged, position));
        if self.dragging != dragged || self.strip_target != target {
            self.dragging = dragged;
            self.strip_target = target;
            cx.notify();
        }
    }

    fn strip_target_at(&self, dragged: Dragged, position: Point<Pixels>) -> Option<(usize, usize)> {
        let geometry = self.geometry.borrow();
        self.layout
            .panes()
            .iter()
            .enumerate()
            .find_map(|(index, pane)| {
                let strip = geometry.strips.get(&pane.id)?;
                if !strip.contains(&position) || !strip_accepts(dragged, index, pane.tabs.len()) {
                    return None;
                }
                let scroll = self.scrolls.get(&pane.id)?;
                let shift = scroll.offset().x;
                let edges: Vec<(Pixels, Pixels)> = (0..pane.tabs.len())
                    .filter_map(|tab| scroll.bounds_for_item(tab))
                    .map(|bounds| (bounds.left() + shift, bounds.size.width))
                    .collect();
                Some((index, strip_gap_at(&edges, position.x)))
            })
    }

    pub(crate) fn strip_gap(&self, pane: usize) -> Option<usize> {
        let (target, gap) = self.strip_target.filter(|(target, _)| *target == pane)?;
        let dragged = self.dragging?;
        Some(strip_drop_gap(
            dragged,
            target,
            &self.layout.panes()[target].tabs,
            gap,
        ))
    }

    fn end_drag(&mut self) -> Option<(usize, usize)> {
        self.dragging = None;
        self.strip_target.take()
    }

    pub(crate) fn drop_tab_on_strip(
        &mut self,
        payload: &DraggedTab,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let target = self.end_drag();
        let (Some((pane, gap)), Some((source, tab))) = (target, self.position_of(payload.item))
        else {
            cx.notify();
            return;
        };
        if source == pane {
            self.layout.move_within(pane, tab, gap);
        } else {
            self.layout.move_to_pane(source, tab, pane, Some(gap));
        }
        self.focus_active(window, cx);
        self.changed(window, cx);
    }

    pub(crate) fn drop_files_on_strip(
        &mut self,
        payload: &DraggedFiles,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let target = self.end_drag();
        let Some((pane, gap)) = target.filter(|_| !payload.has_folder) else {
            cx.notify();
            return;
        };
        self.layout.focus_pane(pane);
        let gap = strip_drop_gap(
            Dragged::files(payload),
            pane,
            &self.layout.panes()[pane].tabs,
            gap,
        );
        for (offset, path) in payload.paths.iter().enumerate() {
            cx.emit(PanesEvent::Open {
                path: path.clone(),
                pane,
                at: Some(gap + offset),
            });
        }
        self.changed(window, cx);
    }

    pub(crate) fn drop_tab_on_zone(
        &mut self,
        payload: &DraggedTab,
        pane: usize,
        zone: DropZone,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.end_drag();
        let Some((source, tab)) = self.position_of(payload.item) else {
            cx.notify();
            return;
        };
        match zone {
            DropZone::Center => self.layout.move_to_pane(source, tab, pane, None),
            DropZone::Left => self.layout.split_tab(source, tab, pane, Side::Left),
            DropZone::Right => self.layout.split_tab(source, tab, pane, Side::Right),
        }
        self.focus_active(window, cx);
        self.changed(window, cx);
    }

    pub(crate) fn drop_files_on_zone(
        &mut self,
        payload: &DraggedFiles,
        pane: usize,
        zone: DropZone,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.end_drag();
        if payload.has_folder {
            cx.notify();
            return;
        }
        let mut paths = payload.paths.iter().cloned();
        let side = match zone {
            DropZone::Left => Side::Left,
            DropZone::Right => Side::Right,
            DropZone::Center => {
                self.layout.focus_pane(pane);
                for path in paths {
                    cx.emit(PanesEvent::Open {
                        path,
                        pane,
                        at: None,
                    });
                }
                self.changed(window, cx);
                return;
            }
        };
        let Some(first) = paths.next() else {
            return;
        };
        cx.emit(PanesEvent::OpenSplit {
            path: first,
            pane,
            side,
        });
        let created = match side {
            Side::Left => pane,
            Side::Right => pane + 1,
        };
        for path in paths {
            cx.emit(PanesEvent::Open {
                path,
                pane: created,
                at: None,
            });
        }
        cx.notify();
    }

    pub(crate) fn resize_divider(
        &mut self,
        divider: usize,
        position: Point<Pixels>,
        cx: &mut Context<'_, Self>,
    ) {
        let (left, right) = {
            let geometry = self.geometry.borrow();
            let (Some(left), Some(right)) = (
                geometry.row.get(divider * 2),
                geometry.row.get(divider * 2 + 2),
            ) else {
                return;
            };
            (*left, *right)
        };
        let pair = f32::from(right.right() - left.left());
        let wanted = f32::from(position.x - left.left());
        self.layout.resize(divider, wanted, pair, MIN_PANE_WIDTH);
        self.resizing = Some(divider);
        cx.notify();
    }
}
