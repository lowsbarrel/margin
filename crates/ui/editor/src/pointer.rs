use std::cmp::Ordering;
use std::time::Duration;

use gpui_kit::{
    Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Task, Window, px,
};
use margin_live_preview::{LinkHit, Mode, outside_hidden};
use margin_markdown::Span;
use margin_text_buffer::{Selection, line_span, word_span_at};

use crate::note_editor::{Edited, NoteEditor};
use crate::{context_menu, links};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Granularity {
    Character,
    Word,
    Line,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragKind {
    Replace,
    Extend,
    Add,
}

#[derive(Clone, Debug)]
struct Drag {
    start: Span,
    granularity: Granularity,
    kind: DragKind,
    base: Vec<Selection>,
    primary: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenIn {
    Current,
    NewTab,
}

const AUTOSCROLL_TICK: Duration = Duration::from_millis(16);
const AUTOSCROLL_MAX_STEP: Pixels = px(48.0);

#[derive(Default)]
pub(crate) struct PointerState {
    drag: Option<Drag>,
    pressed: Option<(LinkHit, Point<Pixels>, OpenIn)>,
    goal_x: Option<Pixels>,
    position: Option<Point<Pixels>>,
    autoscroll: Option<Task<()>>,
}

impl PointerState {
    pub(crate) const fn goal_x(&self) -> Option<Pixels> {
        self.goal_x
    }

    pub(crate) fn set_goal_x(&mut self, goal: Option<Pixels>) {
        self.goal_x = goal;
    }

    pub(crate) const fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }
}

fn range_at(text: &str, offset: usize, granularity: Granularity) -> Span {
    match granularity {
        Granularity::Character => Span::empty(offset),
        Granularity::Word => word_span_at(text, offset),
        Granularity::Line => line_span(text, offset),
    }
}

pub(crate) fn mouse_down(
    editor: &mut NoteEditor,
    event: &MouseDownEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if !editor.is_loaded() {
        return;
    }
    let focus = editor.focus_handle_ref().clone();
    window.focus(&focus, cx);
    let hits = editor.hits();
    let Some(mut offset) = hits.offset_at(event.position) else {
        return;
    };
    let mode = editor.session().mode();
    let granularity = match event.click_count {
        0 | 1 => Granularity::Character,
        2 => Granularity::Word,
        _ => Granularity::Line,
    };
    if mode == Mode::LivePreview && granularity == Granularity::Character {
        offset = outside_hidden(
            editor.session().document(),
            offset,
            editor.session().reveal(),
        );
        let open_in = if event.modifiers.secondary(cx) {
            OpenIn::NewTab
        } else {
            OpenIn::Current
        };
        editor.pointer_mut().pressed = hits
            .link_at(event.position)
            .map(|link| (link, event.position, open_in));
    }
    let kind = if event.modifiers.shift {
        DragKind::Extend
    } else if event.modifiers.alt {
        DragKind::Add
    } else {
        DragKind::Replace
    };
    let session = editor.session();
    let start = range_at(session.text(), offset, granularity);
    let base = session.selections().to_vec();
    let primary = session.buffer().primary_index();
    let drag = Drag {
        start,
        granularity,
        kind,
        base,
        primary,
    };
    editor.session_mut().freeze_reveal();
    editor.pointer_mut().goal_x = None;
    editor.pointer_mut().drag = Some(drag.clone());
    if kind == DragKind::Add
        && granularity == Granularity::Character
        && remove_existing(editor, &drag, offset)
    {
        editor.edited(Edited::Selection, window, cx);
        return;
    }
    select(editor, &drag, offset);
    editor.edited(Edited::Selection, window, cx);
}

fn remove_existing(editor: &mut NoteEditor, drag: &Drag, offset: usize) -> bool {
    if drag.base.len() < 2 {
        return false;
    }
    let Some(index) = drag
        .base
        .iter()
        .position(|selection| selection.start() <= offset && offset <= selection.end())
    else {
        return false;
    };
    let mut remaining = drag.base.clone();
    remaining.remove(index);
    let primary = match drag.primary.cmp(&index) {
        Ordering::Equal => 0,
        Ordering::Greater => drag.primary - 1,
        Ordering::Less => drag.primary,
    };
    editor.session_mut().set_selections(remaining, primary);
    editor.pointer_mut().drag = None;
    true
}

fn select(editor: &mut NoteEditor, drag: &Drag, offset: usize) {
    let text = editor.session().text();
    let current = range_at(text, offset, drag.granularity);
    let range = if drag.kind == DragKind::Extend {
        None
    } else if current.start < drag.start.start {
        Some(Selection::new(
            drag.start.end.max(current.end),
            current.start,
        ))
    } else {
        Some(Selection::new(
            drag.start.start,
            current.end.max(drag.start.end),
        ))
    };
    let session = editor.session_mut();
    match (drag.kind, range) {
        (DragKind::Extend, _) | (_, None) => {
            let anchor = drag
                .base
                .get(drag.primary)
                .map_or(0, |selection| selection.anchor);
            let head = if current.start < anchor {
                current.start
            } else {
                current.end
            };
            let mut selections = drag.base.clone();
            if let Some(primary) = selections.get_mut(drag.primary) {
                *primary = Selection::new(anchor, head);
            }
            session.set_selections(selections, drag.primary);
        }
        (DragKind::Add, Some(range)) => {
            session.set_selections(drag.base.clone(), drag.primary);
            session.add_selection(range);
        }
        (DragKind::Replace, Some(range)) => session.select(range),
    }
}

pub(crate) fn mouse_move(
    editor: &mut NoteEditor,
    event: &MouseMoveEvent,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if editor.pointer().drag.is_none() {
        return;
    }
    if event.pressed_button.is_none() {
        finish_drag(editor, window, cx);
        return;
    }
    if let Some((_, origin, _)) = &editor.pointer().pressed
        && moved_far(*origin, event.position)
    {
        editor.pointer_mut().pressed = None;
    }
    editor.pointer_mut().position = Some(event.position);
    extend_drag(editor, event.position, window, cx);
    if overshoot(editor, event.position) != Pixels::ZERO && editor.pointer().autoscroll.is_none() {
        let task = autoscroll(window, cx);
        editor.pointer_mut().autoscroll = Some(task);
    }
}

fn extend_drag(
    editor: &mut NoteEditor,
    position: Point<Pixels>,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let Some(drag) = editor.pointer().drag.clone() else {
        return;
    };
    let Some(offset) = editor.hits().offset_at(position) else {
        return;
    };
    select(editor, &drag, offset);
    editor.edited(Edited::Selection, window, cx);
}

fn overshoot(editor: &NoteEditor, position: Point<Pixels>) -> Pixels {
    let viewport = editor.session().list().viewport_bounds();
    if position.y < viewport.top() {
        position.y - viewport.top()
    } else if position.y > viewport.bottom() {
        position.y - viewport.bottom()
    } else {
        Pixels::ZERO
    }
}

fn autoscroll(window: &Window, cx: &Context<'_, NoteEditor>) -> Task<()> {
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(AUTOSCROLL_TICK).await;
            let scrolling = this.update_in(cx, |editor, window, cx| {
                let Some(position) = editor.pointer().position else {
                    return false;
                };
                let distance = overshoot(editor, position);
                if !editor.pointer().is_dragging() || distance == Pixels::ZERO {
                    return false;
                }
                let step = distance.clamp(-AUTOSCROLL_MAX_STEP, AUTOSCROLL_MAX_STEP);
                editor.session().list().scroll_by(step);
                extend_drag(editor, position, window, cx);
                true
            });
            if !matches!(scrolling, Ok(true)) {
                break;
            }
        }
        drop(this.update(cx, |editor, _| editor.pointer_mut().autoscroll = None));
    })
}

pub(crate) fn mouse_up(
    editor: &mut NoteEditor,
    event: &MouseUpEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let pressed = editor.pointer_mut().pressed.take();
    finish_drag(editor, window, cx);
    if let Some((link, origin, open_in)) = pressed
        && !moved_far(origin, event.position)
    {
        links::open(editor, &link.kind, open_in, window, cx);
    }
}

pub(crate) fn finish_drag(
    editor: &mut NoteEditor,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    editor.pointer_mut().autoscroll = None;
    editor.pointer_mut().position = None;
    if editor.pointer_mut().drag.take().is_some() {
        editor.session_mut().thaw_reveal();
        editor.edited(Edited::Selection, window, cx);
    }
}

pub(crate) fn context_click(
    editor: &mut NoteEditor,
    event: &MouseDownEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if !editor.is_loaded() {
        return;
    }
    let focus = editor.focus_handle_ref().clone();
    window.focus(&focus, cx);
    if let Some(offset) = editor.hits().offset_at(event.position) {
        context_menu::open(editor, event.position, offset, window, cx);
    }
}

fn moved_far(from: Point<Pixels>, to: Point<Pixels>) -> bool {
    (from.x - to.x).abs() > px(3.0) || (from.y - to.y).abs() > px(3.0)
}
