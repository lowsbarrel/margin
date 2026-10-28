#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "crate-private state shared between the canvas modules"
)]

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AppContext, Bounds, Context, Entity, FocusHandle, KeyDownEvent, KeyUpEvent, Modifiers,
    MouseDownEvent, MouseMoveEvent, PinchEvent, Pixels, Point, ScrollEvent, Subscription, Task,
    Window,
};
use margin_breadcrumbs::Breadcrumbs;
use margin_files::FilesService;
use margin_kernel::RelativePath;
use margin_workbench::{CurrentVault, WorkbenchEvent, run_blocking};
use parking_lot::Mutex;

use crate::board::Board;
use crate::gesture::{Gesture, PressButton, PressOutcome};
use crate::model::WorldPoint;
use crate::serialization;
use crate::tool::Tool;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);
const KEY_ZOOM: f64 = 1.2;
const WHEEL_ZOOM_IN: f64 = 1.1;
const WHEEL_ZOOM_OUT: f64 = 0.9;

pub(crate) use crate::canvas_state::{CanvasMenu, TextEdit, ToolbarPopover};

pub struct CanvasView {
    pub(crate) path: RelativePath,
    pub(crate) files: Arc<FilesService>,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) board: Board,
    pub(crate) open_popover: Option<ToolbarPopover>,
    pub(crate) menu: Option<CanvasMenu>,
    pub(crate) bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(crate) cursor: Option<(f64, f64)>,
    pub(crate) text_edit: Option<TextEdit>,
    pub(crate) requested_revision: u64,
    pub(crate) written_revision: u64,
    pub(crate) write_order: Arc<Mutex<u64>>,
    pub(crate) save_error: Option<String>,
    pub(crate) debounce: Option<Task<()>>,
    pub(crate) loading: Option<Task<()>>,
    pub(crate) crumbs: Entity<Breadcrumbs>,
    pub(crate) _crumbs_subscription: Subscription,
}

impl fmt::Debug for CanvasView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanvasView")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

const fn zoom_modifier(modifiers: Modifiers) -> bool {
    modifiers.control || modifiers.platform
}

impl CanvasView {
    pub fn new(path: RelativePath, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let crumbs = cx.new(|cx| Breadcrumbs::new(path.clone(), window, cx));
        let crumbs_subscription = cx.subscribe(&crumbs, |_, _, event: &WorkbenchEvent, cx| {
            cx.emit(event.clone());
        });
        let mut view = Self {
            path,
            crumbs,
            _crumbs_subscription: crumbs_subscription,
            files: Arc::clone(&CurrentVault::expect(cx).files),
            focus_handle: cx.focus_handle(),
            board: Board::default(),
            open_popover: None,
            menu: None,
            bounds: Rc::default(),
            cursor: None,
            text_edit: None,
            requested_revision: 0,
            written_revision: 0,
            write_order: Arc::default(),
            save_error: None,
            debounce: None,
            loading: None,
        };
        view.focus_handle.focus(window, cx);
        view.load(cx);
        view
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.board.revision != self.written_revision
    }

    pub(crate) fn load(&mut self, cx: &Context<'_, Self>) {
        let files = Arc::clone(&self.files);
        let path = self.path.clone();
        let read = run_blocking(cx, move || files.read_file(&path));
        let revision = self.board.revision;
        self.loading = Some(cx.spawn(async move |this, cx| {
            let Ok(bytes) = read.await else {
                return;
            };
            drop(this.update(cx, |view, cx| {
                if view.board.revision != revision {
                    return;
                }
                let Some((drawing, camera)) =
                    serialization::deserialize(&String::from_utf8_lossy(&bytes))
                else {
                    return;
                };
                if *view.board.drawing != drawing || view.board.camera != camera {
                    view.board.load(drawing, camera);
                    cx.notify();
                }
            }));
        }));
    }

    fn after_change(&mut self, revision_before: u64, cx: &mut Context<'_, Self>) {
        if self.board.revision != revision_before {
            self.debounce = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SAVE_DEBOUNCE).await;
                drop(this.update(cx, |view, cx| view.save(cx)));
            }));
        }
        cx.notify();
    }

    pub(crate) fn save(&mut self, cx: &Context<'_, Self>) {
        self.debounce = None;
        let revision = self.board.revision;
        if revision == self.written_revision || revision == self.requested_revision {
            return;
        }
        self.requested_revision = revision;
        let bytes = serialization::serialize(&self.board.drawing, self.board.camera).into_bytes();
        let files = Arc::clone(&self.files);
        let path = self.path.clone();
        let order = Arc::clone(&self.write_order);
        let write = run_blocking(cx, move || {
            let mut newest = order.lock();
            if revision < *newest {
                return Ok(());
            }
            *newest = revision;
            files.write_file(&path, &bytes)
        });
        cx.spawn(async move |this, cx| {
            let result = write.await;
            drop(this.update(cx, |view, cx| {
                match result {
                    Ok(()) => {
                        view.written_revision = view.written_revision.max(revision);
                        cx.emit(WorkbenchEvent::Saved(view.path.clone()));
                    }
                    Err(error) => {
                        view.requested_revision = 0;
                        view.save_error = Some(error.to_string());
                    }
                }
                cx.notify();
            }));
        })
        .detach();
    }

    fn local(&self, position: Point<Pixels>) -> (f64, f64) {
        let origin = self.bounds.get().origin;
        (
            f64::from((position.x - origin.x).as_f32()),
            f64::from((position.y - origin.y).as_f32()),
        )
    }

    fn center(&self) -> (f64, f64) {
        let size = self.bounds.get().size;
        (
            f64::from(size.width.as_f32()) / 2.0,
            f64::from(size.height.as_f32()) / 2.0,
        )
    }

    pub(crate) fn choose_tool(
        &mut self,
        tool: Tool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.board.set_tool(tool);
        self.open_popover = None;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn begin_text(&mut self, at: WorldPoint, window: &mut Window, cx: &mut Context<'_, Self>) {
        let input = cx.new(|cx| InputState::new(window, cx));
        let subscription =
            cx.subscribe_in(&input, window, |view, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => view.commit_text(window, cx),
                InputEvent::Change | InputEvent::Focus => {}
            });
        input.update(cx, |input, cx| input.focus(window, cx));
        self.text_edit = Some(TextEdit {
            at,
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn commit_text(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        let before = self.board.revision;
        let text = edit.input.read(cx).value();
        self.board.add_label(edit.at, &text);
        self.focus_handle.focus(window, cx);
        self.after_change(before, cx);
    }

    pub(crate) fn cancel_text(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.text_edit = None;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn open_menu(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.commit_text(window, cx);
        self.open_popover = None;
        self.menu = Some(CanvasMenu {
            position: event.position,
            confirming: false,
        });
        cx.notify();
    }

    pub(crate) fn close_menu(&mut self, cx: &mut Context<'_, Self>) {
        self.menu = None;
        cx.notify();
    }

    pub(crate) fn confirm_clear(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(menu) = &mut self.menu {
            menu.confirming = true;
        }
        cx.notify();
    }

    pub(crate) fn clear_all(&mut self, cx: &mut Context<'_, Self>) {
        self.menu = None;
        self.command(Board::clear, cx);
    }

    pub(crate) fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        button: PressButton,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.menu.is_some() {
            self.close_menu(cx);
            return;
        }
        self.commit_text(window, cx);
        self.open_popover = None;
        self.focus_handle.focus(window, cx);
        let before = self.board.revision;
        if let PressOutcome::EditText(at) = self.board.press(self.local(event.position), button) {
            self.begin_text(at, window, cx);
            window.prevent_default();
        }
        self.after_change(before, cx);
    }

    pub(crate) fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<'_, Self>) {
        let local = self.local(event.position);
        self.cursor = Some(local);
        let before = self.board.revision;
        if event.pressed_button.is_some() {
            self.board.drag(local);
        }
        self.after_change(before, cx);
    }

    pub(crate) fn mouse_up(&mut self, cx: &mut Context<'_, Self>) {
        if matches!(self.board.gesture, Gesture::Idle) {
            return;
        }
        let before = self.board.revision;
        self.board.release();
        self.after_change(before, cx);
    }

    pub(crate) fn scroll(
        &mut self,
        event: &ScrollEvent,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height());
        let before = self.board.revision;
        let (delta_x, delta_y) = (f64::from(delta.x.as_f32()), f64::from(delta.y.as_f32()));
        if zoom_modifier(event.modifiers) {
            let factor = if delta_y < 0.0 {
                WHEEL_ZOOM_OUT
            } else {
                WHEEL_ZOOM_IN
            };
            self.board.zoom_at(self.local(event.position), factor);
        } else {
            self.board.pan_by(-delta_x, -delta_y);
        }
        self.after_change(before, cx);
    }

    pub(crate) fn pinch(&mut self, event: &PinchEvent, cx: &mut Context<'_, Self>) {
        let before = self.board.revision;
        self.board
            .zoom_at(self.local(event.position), 1.0 + f64::from(event.delta));
        self.after_change(before, cx);
    }

    pub(crate) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.text_edit.is_some() {
            return;
        }
        let keystroke = &event.keystroke;
        if self.menu.is_some() && keystroke.key == "escape" {
            self.close_menu(cx);
            cx.stop_propagation();
            return;
        }
        let before = self.board.revision;
        let center = self.center();
        if keystroke.key == "space" {
            self.board.space_held = true;
        } else if zoom_modifier(keystroke.modifiers) {
            match keystroke.key.as_str() {
                "=" | "+" => self.board.zoom_at(center, KEY_ZOOM),
                "-" | "_" => self.board.zoom_at(center, 1.0 / KEY_ZOOM),
                "0" => self.board.reset_view(),
                "z" if keystroke.modifiers.shift => self.board.redo(),
                "z" => self.board.undo(),
                _ => return,
            }
        } else if keystroke.modifiers.alt || event.is_held {
            return;
        } else if let Some(tool) = Tool::from_key(&keystroke.key) {
            self.choose_tool(tool, window, cx);
        } else {
            return;
        }
        cx.stop_propagation();
        self.after_change(before, cx);
    }

    pub(crate) fn key_up(&mut self, event: &KeyUpEvent, cx: &mut Context<'_, Self>) {
        if event.keystroke.key == "space" {
            self.board.space_held = false;
            cx.notify();
        }
    }

    pub(crate) fn command(&mut self, apply: impl FnOnce(&mut Board), cx: &mut Context<'_, Self>) {
        let before = self.board.revision;
        apply(&mut self.board);
        self.after_change(before, cx);
    }
}
