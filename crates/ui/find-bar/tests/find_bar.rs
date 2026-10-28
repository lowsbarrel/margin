use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::component::init;
use gpui_kit::{Modifiers, TestAppContext};
use margin_find_bar::{CaseSensitivity, FindBar, FindEvent};
use margin_i18n::{Locale, Translator};
use margin_theme::{Appearance, MarginTheme};

#[gpui_kit::test]
fn typing_emits_query_changed_and_case_toggle_reaches_the_query(cx: &mut TestAppContext) {
    cx.update(|cx| {
        init(cx);
        MarginTheme::init(Appearance::Light, cx);
        Translator::init(Some(Locale::English), cx);
    });
    let (bar, cx) = cx.add_window_view(FindBar::new);
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&events);
    cx.update(|window, cx| {
        cx.subscribe(&bar, move |_, event: &FindEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
        bar.update(cx, |bar, cx| bar.focus(window, cx));
    });

    cx.simulate_input("Foo");
    cx.run_until_parked();

    assert!(events.borrow().contains(&FindEvent::QueryChanged));
    let query = bar.read_with(cx, FindBar::query);
    assert_eq!(query.text, "Foo");
    assert_eq!(query.case, CaseSensitivity::Insensitive);

    events.borrow_mut().clear();
    let case = cx
        .debug_bounds("find-case")
        .expect("the case toggle is rendered");
    cx.simulate_click(case.center(), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(events.borrow().as_slice(), [FindEvent::QueryChanged]);
    let query = bar.read_with(cx, FindBar::query);
    assert_eq!(query.case, CaseSensitivity::Sensitive);

    cx.update(|window, cx| bar.update(cx, |bar, cx| bar.focus(window, cx)));
    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();

    assert_eq!(
        events.borrow().as_slice(),
        [
            FindEvent::QueryChanged,
            FindEvent::Next,
            FindEvent::Previous,
            FindEvent::Close
        ]
    );
}
