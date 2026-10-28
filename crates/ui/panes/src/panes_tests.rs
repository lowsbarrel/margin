use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Modifiers,
    MouseButton, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, div,
    point, px,
};
use margin_i18n::Translator;
use margin_kernel::RelativePath;
use margin_theme::{Appearance, MarginTheme};
use margin_workbench::{DraggedFiles, TabContent, TabHandle, WorkbenchEvent};

use crate::drag::Dragged;
use crate::layout::Side;
use crate::panes::{PaneItem, Panes, PanesEvent};

struct Note {
    path: RelativePath,
    focus: FocusHandle,
    flushed: usize,
}

impl Render for Note {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        div().size_full().child(self.path.as_str().to_owned())
    }
}

impl Focusable for Note {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<WorkbenchEvent> for Note {}

impl TabContent for Note {
    fn path(&self) -> &RelativePath {
        &self.path
    }

    fn rename(&mut self, to: RelativePath, _: &mut Context<'_, Self>) {
        self.path = to;
    }

    fn flush(&mut self, _: &mut Context<'_, Self>) {
        self.flushed += 1;
    }

    fn reload(&mut self, _: &mut Context<'_, Self>) {}
}

#[derive(Clone)]
struct Item(Entity<Note>);

impl PaneItem for Item {
    fn handle(&self) -> &dyn TabHandle {
        &self.0
    }
}

type Events = Rc<RefCell<Vec<PanesEvent>>>;

fn path(name: &str) -> RelativePath {
    RelativePath::new(name).expect("valid path")
}

fn setup(cx: &mut TestAppContext) -> (Entity<Panes<Item>>, &mut VisualTestContext, Events) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        MarginTheme::init(Appearance::Light, cx);
        Translator::init(None, cx);
    });
    let (panes, cx) = cx.add_window_view(|_, cx| Panes::new(cx));
    let events: Events = Rc::default();
    let sink = Rc::clone(&events);
    cx.update(|_, cx| {
        cx.subscribe(&panes, move |_, event: &PanesEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (panes, cx, events)
}

fn open(panes: &Entity<Panes<Item>>, cx: &mut VisualTestContext, name: &str) -> Item {
    let wanted = path(name);
    cx.update(|window, cx| {
        panes.update(cx, |panes, cx| {
            panes.open(
                None,
                None,
                &wanted,
                |_, cx| {
                    let path = wanted.clone();
                    Item(cx.new(|cx| Note {
                        path,
                        focus: cx.focus_handle(),
                        flushed: 0,
                    }))
                },
                window,
                cx,
            )
        })
    })
}

fn titles(panes: &Entity<Panes<Item>>, cx: &mut VisualTestContext) -> Vec<Vec<String>> {
    cx.update(|_, cx| {
        let state = panes.read(cx);
        state
            .layout()
            .panes()
            .iter()
            .map(|pane| {
                pane.tabs
                    .iter()
                    .map(|tab| tab.item.0.read(cx).path.as_str().to_owned())
                    .collect()
            })
            .collect()
    })
}

fn drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32)) {
    let start = point(px(from.0), px(from.1));
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    for step in 1..=10_u8 {
        let t = f32::from(step) / 10.0;
        let at = point(
            px((to.0 - from.0).mul_add(t, from.0)),
            px((to.1 - from.1).mul_add(t, from.1)),
        );
        cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::default());
    }
    cx.simulate_mouse_up(
        point(px(to.0), px(to.1)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
}

#[gpui_kit::test]
fn opening_an_open_path_switches_and_closing_flushes_and_remembers(cx: &mut TestAppContext) {
    let (panes, cx, events) = setup(cx);
    let first = open(&panes, cx, "a.md");
    open(&panes, cx, "b.md");
    open(&panes, cx, "a.md");
    assert_eq!(titles(&panes, cx), [["a.md", "b.md"]]);
    let active = cx.update(|_, cx| panes.read(cx).active_position());
    assert_eq!(active, Some((0, 0)));
    cx.update(|window, cx| panes.update(cx, |panes, cx| panes.close(0, 0, window, cx)));
    assert_eq!(titles(&panes, cx), [["b.md"]]);
    assert_eq!(cx.update(|_, cx| first.0.read(cx).flushed), 1);
    events.borrow_mut().clear();
    cx.update(|_, cx| panes.update(cx, Panes::reopen_closed));
    cx.run_until_parked();
    assert_eq!(
        events.borrow().as_slice(),
        [PanesEvent::Open {
            path: path("a.md"),
            pane: 0,
            at: None,
        }]
    );
    assert!(!cx.update(|_, cx| panes.read(cx).can_reopen()));
}

#[gpui_kit::test]
fn closing_the_last_tab_of_a_split_pane_closes_the_pane(cx: &mut TestAppContext) {
    let (panes, cx, _) = setup(cx);
    open(&panes, cx, "a.md");
    let b = open(&panes, cx, "b.md");
    cx.update(|window, cx| {
        panes.update(cx, |panes, cx| {
            panes.layout.split_tab(0, 1, 0, Side::Right);
            panes.changed(window, cx);
        });
    });
    assert_eq!(titles(&panes, cx), [["a.md"], ["b.md"]]);
    cx.update(|window, cx| panes.update(cx, |panes, cx| panes.close(1, 0, window, cx)));
    assert_eq!(titles(&panes, cx), [["a.md"]]);
    assert_eq!(cx.update(|_, cx| b.0.read(cx).flushed), 1);
}

#[gpui_kit::test]
fn dragging_a_tab_onto_the_right_zone_splits_the_pane(cx: &mut TestAppContext) {
    let (panes, cx, _) = setup(cx);
    open(&panes, cx, "a.md");
    open(&panes, cx, "b.md");
    open(&panes, cx, "c.md");
    cx.run_until_parked();
    let width = f32::from(cx.update(|window, _| window.viewport_size().width));
    drag(cx, (14.0, 17.0), (width - 40.0, 300.0));
    assert_eq!(titles(&panes, cx), [vec!["b.md", "c.md"], vec!["a.md"]]);
    let active = cx.update(|_, cx| panes.read(cx).active_position());
    assert_eq!(active, Some((1, 0)));
}

#[gpui_kit::test]
fn dragging_a_tab_along_its_strip_reorders_it(cx: &mut TestAppContext) {
    let (panes, cx, _) = setup(cx);
    open(&panes, cx, "a.md");
    open(&panes, cx, "b.md");
    open(&panes, cx, "c.md");
    cx.run_until_parked();
    let right_edge = cx.update(|_, cx| {
        let state = panes.read(cx);
        let pane = state.layout().panes()[0].id;
        state.scrolls[&pane]
            .bounds_for_item(2)
            .map(|bounds| f32::from(bounds.right()))
    });
    let right_edge = right_edge.expect("the strip was laid out");
    drag(cx, (14.0, 17.0), (right_edge - 5.0, 17.0));
    assert_eq!(titles(&panes, cx), [["b.md", "c.md", "a.md"]]);
}

#[gpui_kit::test]
fn dropping_files_on_a_strip_opens_them_at_the_gap(cx: &mut TestAppContext) {
    let (panes, cx, events) = setup(cx);
    open(&panes, cx, "a.md");
    open(&panes, cx, "b.md");
    events.borrow_mut().clear();
    let payload = DraggedFiles::new(vec![path("x.md"), path("y.md")]);
    cx.update(|window, cx| {
        panes.update(cx, |panes, cx| {
            panes.dragging = Some(Dragged::files(&payload));
            panes.strip_target = Some((0, 1));
            panes.drop_files_on_strip(&payload, window, cx);
        });
    });
    let opened: Vec<PanesEvent> = events
        .borrow()
        .iter()
        .filter(|event| !matches!(event, PanesEvent::Changed))
        .cloned()
        .collect();
    assert_eq!(
        opened,
        [
            PanesEvent::Open {
                path: path("x.md"),
                pane: 0,
                at: Some(1),
            },
            PanesEvent::Open {
                path: path("y.md"),
                pane: 0,
                at: Some(2),
            },
        ]
    );
    let folder = DraggedFiles::new(vec![path("Projects")]).with_folder(true);
    events.borrow_mut().clear();
    cx.update(|window, cx| {
        panes.update(cx, |panes, cx| {
            panes.strip_target = Some((0, 0));
            panes.drop_files_on_strip(&folder, window, cx);
        });
    });
    assert!(
        events
            .borrow()
            .iter()
            .all(|event| !matches!(event, PanesEvent::Open { .. }))
    );
}
