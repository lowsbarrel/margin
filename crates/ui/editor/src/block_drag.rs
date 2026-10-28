use std::time::Duration;

use gpui_kit::{Context, MouseDownEvent, MouseMoveEvent, Pixels, Point, Task, Window, point, px};
use margin_editor_commands::{
    DragBlock, DragKind, DropGap, Side, block_near, draggable_block, drop_beside, drop_item,
    item_level, list_gaps,
};
use margin_live_preview::Mode;
use margin_markdown::Span;
use margin_text_buffer::{EditOrigin, Selection};

use crate::metrics::{self, COLUMN_PADDING, COLUMN_WIDTH, TypeScale};
use crate::note_editor::{Edited, NoteEditor};
use crate::session::HEADER_ROWS;

pub(crate) const GRIP_WIDTH: Pixels = px(18.0);
pub(crate) const GRIP_HEIGHT: Pixels = px(24.0);
const GRIP_GAP: Pixels = px(6.0);
const TABLE_SHIFT: Pixels = px(22.0);
const DRAG_THRESHOLD: f32 = 4.0;
const SCROLL_EDGE: f32 = 48.0;
const SCROLL_STEP: f32 = 16.0;
const SCROLL_TICK: Duration = Duration::from_millis(16);

#[derive(Clone, Debug)]
enum Target {
    Gap { gap: DropGap, level: usize },
    Beside { block: DragBlock, side: Side },
}

#[derive(Clone, Debug)]
struct Plan {
    target: Target,
    at: Point<Pixels>,
}

struct Drag {
    source: DragBlock,
    level: Option<usize>,
    start: Point<Pixels>,
    position: Point<Pixels>,
    plan: Option<Plan>,
    autoscroll: Option<Task<()>>,
}

impl Drag {
    const fn is_moving(&self) -> bool {
        self.autoscroll.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Grip {
    block: DragBlock,
    origin: Point<Pixels>,
}

#[derive(Default)]
pub(crate) struct BlockDrag {
    grip: Option<Grip>,
    drag: Option<Drag>,
}

impl BlockDrag {
    pub(crate) fn is_moving(&self) -> bool {
        self.drag.as_ref().is_some_and(Drag::is_moving)
    }

    pub(crate) fn dims(&self, span: Span) -> bool {
        self.drag.as_ref().is_some_and(|drag| {
            drag.is_moving()
                && drag.source.span.start <= span.start
                && span.start <= drag.source.span.end
        })
    }

    pub(crate) fn grip_origin(&self) -> Option<Point<Pixels>> {
        self.grip.map(|grip| grip.origin)
    }

    pub(crate) fn drop_point(&self) -> Option<Point<Pixels>> {
        self.drag.as_ref()?.plan.as_ref().map(|plan| plan.at)
    }
}

fn enabled(editor: &NoteEditor) -> bool {
    editor.is_loaded() && editor.session().mode() == Mode::LivePreview
}

pub(crate) fn text_edges(editor: &NoteEditor) -> (Pixels, Pixels) {
    let viewport = editor.session().list().viewport_bounds();
    let width = viewport.size.width.min(COLUMN_WIDTH);
    let left = viewport.left() + (viewport.size.width - width) / 2.0 + COLUMN_PADDING;
    (left, left + width - COLUMN_PADDING * 2.0)
}

fn level_x(left: Pixels, level: usize, window: &Window) -> Pixels {
    let step = metrics::list_step().to_pixels(window.rem_size());
    left + step * f32::from(u16::try_from(level.saturating_sub(1)).unwrap_or(u16::MAX))
}

fn row_edge(editor: &NoteEditor, offset: usize, side: Side) -> Option<Pixels> {
    let item = editor.session().row_at_offset(offset)?;
    let bounds = editor.session().list().bounds_for_item(item)?;
    Some(match side {
        Side::Before => bounds.top(),
        Side::After => bounds.bottom(),
    })
}

fn offset_at_y(editor: &NoteEditor, y: Pixels, clamp: bool) -> Option<usize> {
    let session = editor.session();
    let list = session.list();
    let viewport_bottom = list.viewport_bounds().bottom();
    let mut previous = None;
    for item in list.logical_scroll_top().item_ix.max(HEADER_ROWS)..list.item_count() {
        let Some(bounds) = list.bounds_for_item(item) else {
            break;
        };
        if y < bounds.bottom() {
            let hit = clamp || y >= bounds.top();
            return hit
                .then(|| session.row(item))
                .flatten()
                .map(|row| row.span.start);
        }
        previous = Some(item);
        if bounds.bottom() > viewport_bottom {
            break;
        }
    }
    previous
        .filter(|_| clamp)
        .and_then(|item| session.row(item))
        .map(|row| row.span.start)
}

fn grip_at(editor: &NoteEditor, y: Pixels, window: &Window) -> Option<Grip> {
    let document = editor.session().document();
    let block = draggable_block(document, offset_at_y(editor, y, false)?)?;
    let (left, _) = text_edges(editor);
    let edge = match block.kind {
        DragKind::ListItem => level_x(left, item_level(document, &block).unwrap_or(1), window),
        DragKind::Table => left - TABLE_SHIFT,
        DragKind::Block | DragKind::Frontmatter => left,
    };
    let middle = if let Some(caret) = editor.hits().caret_bounds(block.anchor) {
        caret.center().y
    } else {
        let line = TypeScale::Body.line_rems().to_pixels(window.rem_size());
        let item = editor.session().row_at_offset(block.span.start)?;
        let row = editor.session().list().bounds_for_item(item)?;
        row.top() + row.size.height.min(line) / 2.0
    };
    Some(Grip {
        block,
        origin: point(edge - GRIP_WIDTH - GRIP_GAP, middle - GRIP_HEIGHT / 2.0),
    })
}

pub(crate) fn mouse_move(
    editor: &mut NoteEditor,
    event: &MouseMoveEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if editor.block_drag().drag.is_some() {
        drag_to(editor, event, window, cx);
        return;
    }
    let inside = editor
        .session()
        .list()
        .viewport_bounds()
        .contains(&event.position);
    let idle = event.pressed_button.is_none() && !editor.pointer().is_dragging();
    let grip = (inside && idle && enabled(editor))
        .then(|| grip_at(editor, event.position.y, window))
        .flatten();
    if grip != editor.block_drag().grip {
        editor.block_drag_mut().grip = grip;
        cx.notify();
    }
}

pub(crate) fn press(
    editor: &mut NoteEditor,
    event: &MouseDownEvent,
    _: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    cx.stop_propagation();
    let Some(grip) = editor.block_drag().grip else {
        return;
    };
    if editor.block_drag().drag.is_some() || !enabled(editor) {
        return;
    }
    let level = item_level(editor.session().document(), &grip.block);
    editor.block_drag_mut().drag = Some(Drag {
        source: grip.block,
        level,
        start: event.position,
        position: event.position,
        plan: None,
        autoscroll: None,
    });
}

fn drag_to(
    editor: &mut NoteEditor,
    event: &MouseMoveEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if event.pressed_button.is_none() {
        finish(editor, true, window, cx);
        return;
    }
    let Some(drag) = editor.block_drag_mut().drag.as_mut() else {
        return;
    };
    drag.position = event.position;
    if drag.autoscroll.is_none() {
        let offset = drag.position - drag.start;
        if f32::hypot(offset.x / px(1.0), offset.y / px(1.0)) < DRAG_THRESHOLD {
            return;
        }
        drag.autoscroll = Some(autoscroll(window, cx));
        let focus = editor.focus_handle_ref().clone();
        window.focus(&focus, cx);
    }
    replan(editor, window);
    cx.notify();
}

fn replan(editor: &mut NoteEditor, window: &Window) {
    let plan = editor
        .block_drag()
        .drag
        .as_ref()
        .and_then(|drag| plan_at(editor, drag, window));
    if let Some(drag) = editor.block_drag_mut().drag.as_mut() {
        drag.plan = plan;
    }
}

fn plan_at(editor: &NoteEditor, drag: &Drag, window: &Window) -> Option<Plan> {
    let document = editor.session().document();
    let pos = offset_at_y(editor, drag.position.y, true)?;
    let (left, _) = text_edges(editor);
    if let Some(level) = drag.level {
        let nearest = list_gaps(document, pos, &drag.source)
            .into_iter()
            .filter_map(|gap| row_edge(editor, gap.pos, gap.side).map(|y| (gap, y)))
            .min_by_key(|(_, y)| (*y - drag.position.y).abs());
        if let Some((gap, y)) = nearest {
            let goal = level_x(left, level, window) + (drag.position.x - drag.start.x);
            let level = (gap.min_level..=gap.max_level)
                .min_by_key(|candidate| (level_x(left, *candidate, window) - goal).abs())
                .unwrap_or(gap.min_level);
            let at = point(level_x(left, level, window), y);
            return Some(Plan {
                target: Target::Gap { gap, level },
                at,
            });
        }
    }
    let (block, side) = block_near(document, pos)?;
    let source = drag.source.span;
    if block.span.start <= source.end && source.start <= block.span.end {
        return None;
    }
    let top = row_edge(editor, block.span.start, Side::Before);
    let bottom = row_edge(editor, block.span.end, Side::After);
    let after = match (block.kind, side, top, bottom) {
        (DragKind::Frontmatter, ..) | (_, Some(Side::After), ..) => true,
        (_, Some(Side::Before), ..) => false,
        (_, None, Some(top), Some(bottom)) => drag.position.y > (top + bottom) / 2.0,
        (_, None, top, _) => top.is_none(),
    };
    let (side, y) = if after {
        (Side::After, bottom?)
    } else {
        (Side::Before, top?)
    };
    Some(Plan {
        target: Target::Beside { block, side },
        at: point(left, y),
    })
}

fn autoscroll(window: &Window, cx: &Context<'_, NoteEditor>) -> Task<()> {
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(SCROLL_TICK).await;
            let alive = this.update_in(cx, |editor, window, cx| {
                let Some(y) = editor
                    .block_drag()
                    .drag
                    .as_ref()
                    .map(|drag| drag.position.y)
                else {
                    return false;
                };
                let viewport = editor.session().list().viewport_bounds();
                let above = (y - viewport.top()) / px(1.0);
                let below = (viewport.bottom() - y) / px(1.0);
                let step = if above < SCROLL_EDGE {
                    -SCROLL_STEP * (1.0 - above.max(0.0) / SCROLL_EDGE)
                } else if below < SCROLL_EDGE {
                    SCROLL_STEP * (1.0 - below.max(0.0) / SCROLL_EDGE)
                } else {
                    return true;
                };
                editor.session().list().scroll_by(px(step));
                replan(editor, window);
                cx.notify();
                true
            });
            if !matches!(alive, Ok(true)) {
                break;
            }
        }
    })
}

pub(crate) fn release(
    editor: &mut NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    finish(editor, true, window, cx);
}

pub(crate) fn cancel(
    editor: &mut NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) -> bool {
    editor.block_drag().is_moving() && finish(editor, false, window, cx)
}

fn finish(
    editor: &mut NoteEditor,
    apply: bool,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) -> bool {
    let Some(drag) = editor.block_drag_mut().drag.take() else {
        return false;
    };
    if !drag.is_moving() {
        if apply {
            let span = drag.source.span;
            let focus = editor.focus_handle_ref().clone();
            window.focus(&focus, cx);
            editor
                .session_mut()
                .select(Selection::new(span.start, span.end));
            editor.edited(Edited::Selection, window, cx);
        }
        return true;
    }
    editor.block_drag_mut().grip = None;
    let plan = apply.then(|| plan_at(editor, &drag, window)).flatten();
    let session = editor.session();
    let (document, selections) = (session.document(), session.selections());
    let change = plan.and_then(|plan| match &plan.target {
        Target::Gap { gap, level } => drop_item(document, selections, &drag.source, gap, *level),
        Target::Beside { block, side } => {
            drop_beside(document, selections, &drag.source, block, *side)
        }
    });
    match change {
        Some(change) => {
            editor.apply(change, EditOrigin::Command, window, cx);
        }
        None => cx.notify(),
    }
    true
}
