use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::TermMode;
use futures_channel::mpsc::UnboundedReceiver;
use futures_util::StreamExt;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Render, ScrollEvent, Styled, Subscription, Task, Window,
    div,
};
use margin_terminals::{SessionId, Terminals};
use margin_theme::MarginTheme;

use crate::colors::{Focus, TerminalColors};
use crate::emulator::TermRequest;
use crate::grid_layout::GridLayout;
use crate::keys::{self, AltKey};
use crate::pointer::{Pointer, PointerOutcome};
use crate::scroll_handle::ScrollHandle;
use crate::session::{Screen, Session, Wake};
use crate::session_status::{SessionStatus, StatusChanged};
use crate::terminal_element::TerminalElement;

pub(crate) const KEY_CONTEXT: &str = "Terminal";
const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(600);

pub(crate) struct TerminalView {
    id: SessionId,
    terminals: Terminals,
    cwd: PathBuf,
    screen: Arc<Screen>,
    session: Option<Session>,
    status: SessionStatus,
    focus_handle: FocusHandle,
    layout: Rc<Cell<GridLayout>>,
    scroll_handle: ScrollHandle,
    pointer: Pointer,
    marked_text: Option<String>,
    cursor_visible: bool,
    blink_epoch: usize,
    blink: Task<()>,
    _pump: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<StatusChanged> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TerminalView {
    pub(crate) fn new(
        id: SessionId,
        terminals: Terminals,
        cwd: PathBuf,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let layout = Rc::new(Cell::new(GridLayout::default()));
        let (screen, woken) = Screen::new(layout.get().terminal_size());
        let focus_handle = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus_in(&focus_handle, window, |view, window, cx| {
                view.focus_changed(Focus::Focused, window, cx);
            }),
            cx.on_blur(&focus_handle, window, |view, window, cx| {
                view.focus_changed(Focus::Unfocused, window, cx);
            }),
            cx.observe_window_activation(window, |view, window, cx| {
                if view.focus_handle.is_focused(window) && window.is_window_active() {
                    view.restart_blink(window, cx);
                }
            }),
        ];
        Self {
            id,
            terminals,
            cwd,
            scroll_handle: ScrollHandle::new(Arc::clone(&screen), Rc::clone(&layout)),
            _pump: Self::pump(woken, cx),
            screen,
            session: None,
            status: SessionStatus::Running,
            focus_handle,
            layout,
            pointer: Pointer::new(),
            marked_text: None,
            cursor_visible: true,
            blink_epoch: 0,
            blink: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    pub(crate) const fn status(&self) -> &SessionStatus {
        &self.status
    }

    fn pump(mut woken: UnboundedReceiver<Wake>, cx: &Context<'_, Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            while let Some(wake) = woken.next().await {
                if this.update(cx, |view, cx| view.wake(wake, cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn wake(&mut self, wake: Wake, cx: &mut Context<'_, Self>) {
        match wake {
            Wake::Output => {
                self.screen.output_seen();
                let requests = self.screen.lock().take_requests();
                for request in requests {
                    self.answer(request, cx);
                }
            }
            Wake::Exited(code) => {
                self.status = SessionStatus::Exited(code);
                cx.emit(StatusChanged);
            }
            Wake::Failed(error) => {
                self.status = SessionStatus::Failed(error);
                cx.emit(StatusChanged);
            }
        }
        cx.notify();
    }

    fn answer(&self, request: TermRequest, cx: &App) {
        let reply = match request {
            TermRequest::Reply(text) => text,
            TermRequest::Copy(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                return;
            }
            TermRequest::Color { index, format } => {
                let colors = TerminalColors::new(MarginTheme::palette(cx));
                let queried = colors.queried(index, self.screen.lock().term().colors());
                let Some(rgb) = queried else {
                    return;
                };
                format(rgb)
            }
            TermRequest::TextAreaSize(format) => format(self.layout.get().window_size()),
        };
        if let Some(session) = &self.session {
            session.write(reply.into_bytes());
        }
    }

    pub(crate) fn apply_layout(&mut self, layout: GridLayout, cx: &App) {
        self.layout.set(layout);
        let size = layout.terminal_size();
        let resized = {
            let mut emulator = self.screen.lock();
            let resized = emulator.size() != size;
            emulator.resize(size);
            resized
        };
        match &self.session {
            Some(session) if resized => session.resize(size),
            Some(_) => {}
            None => {
                self.session = Some(Session::start(
                    self.id,
                    self.terminals.clone(),
                    self.cwd.clone(),
                    size,
                    Arc::clone(&self.screen),
                    cx,
                ));
            }
        }
    }

    pub(crate) fn input(&mut self, bytes: Vec<u8>, window: &Window, cx: &mut Context<'_, Self>) {
        let Some(session) = self
            .session
            .as_ref()
            .filter(|_| self.status == SessionStatus::Running)
        else {
            return;
        };
        {
            let mut emulator = self.screen.lock();
            emulator.scroll_page(Scroll::Bottom);
            emulator.clear_selection();
        }
        session.write(bytes);
        self.restart_blink(window, cx);
    }

    pub(crate) fn set_marked_text(&mut self, text: Option<String>, cx: &mut Context<'_, Self>) {
        self.marked_text = text.filter(|text| !text.is_empty());
        cx.notify();
    }

    pub(crate) fn marked_text(&self) -> Option<&str> {
        self.marked_text.as_deref()
    }

    pub(crate) fn cursor_bounds(&self) -> Bounds<Pixels> {
        let emulator = self.screen.lock();
        let cursor = emulator.term().grid().cursor.point;
        let row = usize::try_from(cursor.line.0).unwrap_or(0) + emulator.display_offset();
        self.layout.get().cell_bounds(cursor.column.0, row, 1)
    }

    fn focus_changed(&mut self, focus: Focus, window: &Window, cx: &mut Context<'_, Self>) {
        let report = {
            let mut emulator = self.screen.lock();
            emulator.set_focused(focus == Focus::Focused);
            emulator.mode().contains(TermMode::FOCUS_IN_OUT)
        };
        if report && let Some(session) = &self.session {
            let sequence = match focus {
                Focus::Focused => b"\x1b[I",
                Focus::Unfocused => b"\x1b[O",
            };
            session.write(sequence.to_vec());
        }
        match focus {
            Focus::Focused => self.restart_blink(window, cx),
            Focus::Unfocused => {
                self.blink_epoch += 1;
                self.blink = Task::ready(());
                self.cursor_visible = true;
                cx.notify();
            }
        }
    }

    fn restart_blink(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        self.blink_epoch += 1;
        self.cursor_visible = true;
        cx.notify();
        let epoch = self.blink_epoch;
        self.blink = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(CURSOR_BLINK_INTERVAL).await;
                let blinking = this.update_in(cx, |view, window, cx| {
                    let current = view.blink_epoch == epoch && window.is_window_active();
                    view.cursor_visible = !current || !view.cursor_visible;
                    cx.notify();
                    current
                });
                if !matches!(blinking, Ok(true)) {
                    break;
                }
            }
        });
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<'_, Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        let mac = cx.platform_info().is_mac();
        let command = modifiers.secondary(cx) && !modifiers.alt;
        if command && keystroke.key == "c" {
            let selection = self.screen.lock().selection_text();
            if let Some(text) = selection {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                cx.stop_propagation();
                return;
            }
            if mac {
                return;
            }
        }
        let mode = self.screen.lock().mode();
        if command && keystroke.key == "v" && !modifiers.shift {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.input(keys::paste_bytes(&text, mode), window, cx);
            }
            cx.stop_propagation();
            return;
        }
        if mac && keystroke.key == "v" && modifiers == gpui_kit::Modifiers::control() {
            cx.stop_propagation();
            return;
        }
        if let Some(scroll) = keys::scrollback_key(keystroke, mode) {
            self.screen.lock().scroll_page(scroll);
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if event.prefer_character_input && keystroke.key_char.is_some() {
            return;
        }
        let alt = if mac {
            AltKey::ComposesCharacters
        } else {
            AltKey::Meta
        };
        if let Some(bytes) = keys::key_bytes(keystroke, mode, alt) {
            self.input(bytes, window, cx);
            cx.stop_propagation();
        }
    }

    fn mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let mac = cx.platform_info().is_mac();
        let outcome = self
            .pointer
            .down(&self.screen, self.layout.get(), event, mac);
        self.apply(outcome, window, cx);
    }

    fn mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let outcome = self.pointer.moved(&self.screen, self.layout.get(), event);
        self.apply(outcome, window, cx);
    }

    fn mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<'_, Self>) {
        let outcome = self.pointer.up(&self.screen, self.layout.get(), event);
        self.apply(outcome, window, cx);
    }

    fn scroll_wheel(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        cx.stop_propagation();
        let outcome = self.pointer.wheel(&self.screen, self.layout.get(), event);
        self.apply(outcome, window, cx);
    }

    fn apply(&mut self, outcome: PointerOutcome, window: &Window, cx: &mut Context<'_, Self>) {
        match outcome {
            PointerOutcome::Ignored => {}
            PointerOutcome::Redraw => cx.notify(),
            PointerOutcome::Send(bytes) => self.input(bytes, window, cx),
        }
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let focus = if self.focus_handle.is_focused(window) {
            Focus::Focused
        } else {
            Focus::Unfocused
        };
        let element = TerminalElement::new(
            cx.entity(),
            Arc::clone(&self.screen),
            self.focus_handle.clone(),
            focus,
            self.cursor_visible,
            TerminalColors::new(palette),
        );
        div()
            .id("terminal")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .bg(palette.surface(0))
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::mouse_up))
            .on_scroll(cx.listener(Self::scroll_wheel))
            .child(div().size_full().px_3().py_2().child(element))
            .child(Scrollbar::vertical(&self.scroll_handle))
            .children(self.status.overlay(palette, cx))
    }
}
