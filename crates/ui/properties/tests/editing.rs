use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::{Focusable, TestAppContext, VisualTestContext};
use margin_i18n::{Locale, Translator};
use margin_properties::{PropertiesEvent, PropertiesPanel};
use margin_theme::{Appearance, MarginTheme};

const NOTE: &str = "---\ntitle: Roadmap\ndone: false\n---\n# Body\n";

fn open_panel<'test>(
    cx: &'test mut TestAppContext,
    note: &'static str,
) -> (
    Rc<RefCell<Vec<PropertiesEvent>>>,
    &'test mut VisualTestContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Translator::init(Some(Locale::English), cx);
        MarginTheme::init(Appearance::Light, cx);
    });
    let (panel, cx) = cx.add_window_view(PropertiesPanel::new);
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&events);
    cx.update(|window, cx| {
        cx.subscribe(&panel, move |_, event: &PropertiesEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
        panel.update(cx, |panel, cx| panel.set_source(note, cx));
        window.focus(&panel.focus_handle(cx), cx);
    });
    cx.run_until_parked();
    (events, cx)
}

#[gpui_kit::test]
fn editing_a_text_value_emits_the_new_source(cx: &mut TestAppContext) {
    let (events, cx) = open_panel(cx, NOTE);
    cx.simulate_keystrokes("down enter");
    cx.simulate_input("Launch");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        events.borrow().as_slice(),
        [PropertiesEvent::Changed(
            "---\ntitle: Launch\ndone: false\n---\n# Body\n".to_owned()
        )]
    );
}

#[gpui_kit::test]
fn escape_discards_a_text_edit(cx: &mut TestAppContext) {
    let (events, cx) = open_panel(cx, NOTE);
    cx.simulate_keystrokes("down enter");
    cx.simulate_input("Launch");
    cx.simulate_keystrokes("escape j");
    cx.run_until_parked();
    assert!(events.borrow().is_empty());
}

#[gpui_kit::test]
fn deleting_a_focused_row_removes_the_property(cx: &mut TestAppContext) {
    let (events, cx) = open_panel(cx, NOTE);
    cx.simulate_keystrokes("down j delete");
    cx.run_until_parked();
    assert_eq!(
        events.borrow().as_slice(),
        [PropertiesEvent::Changed(
            "---\ntitle: Roadmap\n---\n# Body\n".to_owned()
        )]
    );
}

#[gpui_kit::test]
fn a_tag_typed_into_the_list_is_appended(cx: &mut TestAppContext) {
    let (events, cx) = open_panel(cx, "---\ntags:\n  - plan\n---\nBody\n");
    cx.simulate_keystrokes("down enter");
    cx.simulate_input("#ops");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        events.borrow().as_slice(),
        [PropertiesEvent::Changed(
            "---\ntags:\n  - plan\n  - ops\n---\nBody\n".to_owned()
        )]
    );
}

#[gpui_kit::test]
fn a_new_property_is_added_from_the_keyboard(cx: &mut TestAppContext) {
    let (events, cx) = open_panel(cx, NOTE);
    cx.simulate_keystrokes("down o");
    cx.simulate_input("owner");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        events.borrow().as_slice(),
        [PropertiesEvent::Changed(
            "---\ntitle: Roadmap\ndone: false\nowner:\n---\n# Body\n".to_owned()
        )]
    );
}
