use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext,
    VisualTestContext, component, point, px, size,
};
use margin_i18n::{Locale, Translator};
use margin_terminal::{CloseTerminal, TerminalDock, TerminalDockEvent};
use margin_terminals::Terminals;
use margin_test_support::{FakePtyHost, FakeSession};
use margin_theme::{Appearance, MarginTheme};

const VAULT: &str = "/vault";

fn open_dock<'cx>(
    cx: &'cx mut TestAppContext,
    host: &FakePtyHost,
) -> (Entity<TerminalDock>, &'cx mut VisualTestContext) {
    cx.update(|cx| {
        component::init(cx);
        MarginTheme::init(Appearance::Light, cx);
        Translator::init(Some(Locale::English), cx);
    });
    let terminals = Terminals::new(Arc::new(host.clone()));
    let (dock, cx) = cx.add_window_view(|window, cx| {
        TerminalDock::with_terminals(terminals, PathBuf::from(VAULT), window, cx)
    });
    cx.simulate_resize(size(px(800.), px(400.)));
    dock.update_in(cx, TerminalDock::new_terminal);
    redraw(cx);
    (dock, cx)
}

fn redraw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

fn shell(host: &FakePtyHost) -> FakeSession {
    host.last_session().expect("the dock started a shell")
}

fn double_click(cx: &mut VisualTestContext, x: f32, y: f32) {
    let position = point(px(x), px(y));
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
}

#[gpui_kit::test]
fn typing_reaches_the_shell_started_in_the_vault(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (_dock, cx) = open_dock(cx, &host);
    let session = shell(&host);
    assert_eq!(session.cwd(), Path::new(VAULT));
    assert!(
        session.size().cols() > 20,
        "the shell starts at the laid-out width"
    );
    cx.simulate_input("ls");
    cx.simulate_keystrokes("enter up ctrl-a");
    assert_eq!(session.written(), b"ls\r\x1b[A\x01".to_vec());
}

#[gpui_kit::test]
fn shell_output_is_drawn_and_can_be_copied(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (_dock, cx) = open_dock(cx, &host);
    let session = shell(&host);
    session.output(b"\x1b[999;1H\x1b[1;32mhello\x1b[0m world");
    redraw(cx);
    double_click(cx, 1., 399.);
    cx.simulate_keystrokes("secondary-c");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("hello"));
    assert!(
        !session.written().contains(&3),
        "copying a selection must not interrupt the shell"
    );
}

#[gpui_kit::test]
fn the_emulator_answers_shell_queries(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (_dock, cx) = open_dock(cx, &host);
    let session = shell(&host);
    session.output(b"abc\x1b[6n");
    redraw(cx);
    assert_eq!(session.written(), b"\x1b[1;4R".to_vec());
}

#[gpui_kit::test]
fn resizing_the_window_resizes_the_pty(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (_dock, cx) = open_dock(cx, &host);
    let started = shell(&host).size();
    cx.simulate_resize(size(px(500.), px(300.)));
    redraw(cx);
    let resizes = shell(&host).resizes();
    let last = resizes.last().expect("the shell was resized");
    assert!(last.cols() < started.cols() && last.rows() < started.rows());
}

#[gpui_kit::test]
fn an_exited_shell_ignores_input_and_closing_the_last_tab_hides_the_dock(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (dock, cx) = open_dock(cx, &host);
    let events = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&events);
    cx.update(|_, cx| {
        cx.subscribe(&dock, move |_, event: &TerminalDockEvent, _| {
            recorded.borrow_mut().push(*event);
        })
        .detach();
    });
    let session = shell(&host);
    session.exit(0);
    redraw(cx);
    cx.simulate_input("x");
    assert!(session.written().is_empty());
    cx.dispatch_action(CloseTerminal);
    redraw(cx);
    assert!(!dock.read_with(cx, |dock, _| dock.has_terminals()));
    assert_eq!(*events.borrow(), vec![TerminalDockEvent::Hide]);
}

#[gpui_kit::test]
fn closing_a_running_terminal_kills_its_shell_and_focuses_the_neighbour(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (dock, cx) = open_dock(cx, &host);
    dock.update_in(cx, TerminalDock::new_terminal);
    redraw(cx);
    let second = shell(&host);
    cx.dispatch_action(CloseTerminal);
    redraw(cx);
    assert!(second.killed());
    assert!(dock.read_with(cx, |dock, _| dock.has_terminals()));
    cx.simulate_input("y");
    let first = host.session(0).expect("the first shell is still running");
    assert_eq!(first.written(), b"y".to_vec());
}
