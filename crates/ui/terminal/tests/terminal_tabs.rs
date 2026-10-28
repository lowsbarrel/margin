use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{
    Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, component, point, px, size,
};
use margin_i18n::{Locale, Translator};
use margin_terminal::TerminalDock;
use margin_terminals::Terminals;
use margin_test_support::FakePtyHost;
use margin_theme::{Appearance, MarginTheme};

fn open_dock<'cx>(
    cx: &'cx mut TestAppContext,
    host: &FakePtyHost,
    tabs: usize,
) -> (Entity<TerminalDock>, &'cx mut VisualTestContext) {
    cx.update(|cx| {
        component::init(cx);
        MarginTheme::init(Appearance::Light, cx);
        Translator::init(Some(Locale::English), cx);
    });
    let terminals = Terminals::new(Arc::new(host.clone()));
    let (dock, cx) = cx.add_window_view(|window, cx| {
        TerminalDock::with_terminals(terminals, PathBuf::from("/vault"), window, cx)
    });
    cx.simulate_resize(size(px(800.), px(400.)));
    for _ in 0..tabs {
        dock.update_in(cx, TerminalDock::new_terminal);
        redraw(cx);
    }
    (dock, cx)
}

fn redraw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn clicking_a_tab_activates_and_focuses_its_shell(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (_dock, cx) = open_dock(cx, &host, 2);
    cx.simulate_click(point(px(30.), px(18.)), Modifiers::none());
    redraw(cx);
    cx.simulate_input("a");
    let first = host.session(0).expect("the first shell runs");
    let second = host.session(1).expect("the second shell runs");
    assert_eq!(first.written(), b"a".to_vec());
    assert!(second.written().is_empty());
}

#[gpui_kit::test]
fn ctrl_v_on_macos_never_reaches_the_shell(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (_dock, cx) = open_dock(cx, &host, 1);
    let mac = cx.update(|_, cx| cx.platform_info().is_mac());
    cx.simulate_keystrokes("ctrl-v");
    let written = host.session(0).expect("the shell runs").written();
    assert_eq!(written.is_empty(), mac);
}

#[gpui_kit::test]
fn right_click_on_macos_selects_the_word_like_xterm(cx: &mut TestAppContext) {
    let host = FakePtyHost::new();
    let (_dock, cx) = open_dock(cx, &host, 1);
    let mac = cx.update(|_, cx| cx.platform_info().is_mac());
    host.session(0)
        .expect("the shell runs")
        .output(b"\x1b[999;1Hhello world");
    redraw(cx);
    cx.simulate_mouse_down(
        point(px(1.), px(399.)),
        MouseButton::Right,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(1.), px(399.)),
        MouseButton::Right,
        Modifiers::none(),
    );
    cx.simulate_keystrokes("secondary-c");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), mac.then_some("hello"));
}
