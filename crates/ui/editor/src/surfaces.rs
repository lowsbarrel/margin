use gpui_kit::{
    Anchor, AnyElement, App, AppContext, Context, Div, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Stateful, Styled, Subscription,
    Window, anchored, deferred, div, point, px,
};
use margin_editor_assets::{DiagramCache, VaultIndex};
use margin_editor_commands::{active_marks, block_type, inside_code, set_block, set_link};
use margin_editor_overlays::{
    BubbleEvent, BubbleState, BubbleToolbar, CompletionMenu, ImageLightbox, LightboxEvent,
    LightboxImage, completion_trigger,
};
use margin_live_preview::{Mode, RowKind};
use margin_properties::{PropertiesEvent, PropertiesPanel};
use margin_table_editing::{Direction, cell_at, leave_table};
use margin_text_buffer::EditOrigin;
use margin_theme::MarginTheme;

use crate::completion::{self, Completing};
use crate::context_menu::OpenMenu;
use crate::find::{self, FindState};
use crate::note_editor::{Edited, NoteEditor};
use crate::{block_drag, context_menu, editing, metrics, properties};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuKey {
    Up,
    Down,
    Confirm,
}

pub(crate) struct Surfaces {
    find: FindState,
    properties: Entity<PropertiesPanel>,
    frontmatter: Option<String>,
    completing: Completing,
    bubble: Entity<BubbleToolbar>,
    lightbox: Option<(Entity<ImageLightbox>, Subscription)>,
    context_menu: Option<OpenMenu>,
    bubble_suppressed: bool,
    _subscriptions: Vec<Subscription>,
}

impl Surfaces {
    pub(crate) fn new(
        focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<'_, NoteEditor>,
    ) -> Self {
        let properties = cx.new(|cx| PropertiesPanel::new(window, cx));
        let bubble = cx.new(|cx| BubbleToolbar::new(focus.clone(), cx));
        let menu = cx.new(CompletionMenu::new);
        let subscriptions = vec![
            cx.subscribe_in(
                &properties,
                window,
                |editor, _, event: &PropertiesEvent, window, cx| {
                    properties::on_event(editor, event, window, cx);
                },
            ),
            cx.subscribe_in(
                &bubble,
                window,
                |editor, _, event: &BubbleEvent, window, cx| {
                    on_bubble(editor, event, window, cx);
                },
            ),
            cx.subscribe_in(&menu, window, |editor, _, event, window, cx| {
                completion::on_event(editor, event, window, cx);
            }),
            cx.observe_global::<DiagramCache>(|_, cx| cx.notify()),
            cx.observe_global::<VaultIndex>(|editor, cx| {
                editor.surfaces().completing.refresh_sources(cx);
                cx.notify();
            }),
            cx.observe_global::<MarginTheme>(|_, cx| cx.notify()),
        ];
        Self {
            find: FindState::default(),
            properties,
            frontmatter: None,
            completing: Completing::new(menu),
            bubble,
            lightbox: None,
            context_menu: None,
            bubble_suppressed: false,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn find_highlights(
        &self,
    ) -> (Vec<margin_markdown::Span>, Option<margin_markdown::Span>) {
        self.find.highlights()
    }

    pub(crate) const fn find(&self) -> &FindState {
        &self.find
    }

    pub(crate) fn find_mut(&mut self) -> &mut FindState {
        &mut self.find
    }

    pub(crate) const fn completing(&self) -> &Completing {
        &self.completing
    }

    pub(crate) fn completing_mut(&mut self) -> &mut Completing {
        &mut self.completing
    }

    pub(crate) fn set_context_menu(&mut self, menu: Option<OpenMenu>) {
        self.context_menu = menu;
    }
}

pub(crate) fn after_edit(
    editor: &mut NoteEditor,
    kind: Edited,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    editor.surfaces_mut().bubble_suppressed = false;
    if kind == Edited::Text {
        let frontmatter = editor
            .session()
            .document()
            .frontmatter()
            .map(|frontmatter| frontmatter.span.slice(editor.session().text()).to_owned());
        if frontmatter != editor.surfaces().frontmatter {
            let source = editor.session().text().to_owned();
            editor
                .surfaces()
                .properties
                .update(cx, |panel, cx| panel.set_source(&source, cx));
            editor.surfaces_mut().frontmatter = frontmatter;
        }
        find::refresh(editor, cx);
    }
    let focused = editor.focus_handle_ref().is_focused(window);
    let session = editor.session();
    let trigger = (focused && session.selections().len() == 1 && session.primary().is_empty())
        .then(|| completion_trigger(session.document(), session.primary().head))
        .flatten();
    completion::update(editor, trigger, kind, cx);
    let session = editor.session();
    let primary = session.primary();
    if !primary.is_empty() && session.selections().len() == 1 {
        let state = BubbleState {
            marks: active_marks(session.document(), primary),
            block: block_type(session.document(), primary),
            in_table: cell_at(session.document(), primary.head).is_some(),
        };
        editor
            .surfaces()
            .bubble
            .update(cx, |bubble, cx| bubble.set_state(state, cx));
    }
}

fn on_bubble(
    editor: &mut NoteEditor,
    event: &BubbleEvent,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    match event {
        BubbleEvent::SetLink(url) => {
            editing::apply_change(
                editor,
                EditOrigin::Command,
                window,
                cx,
                |document, selections, primary| set_link(document, selections, primary, url),
            );
        }
        BubbleEvent::CancelLink => {}
        BubbleEvent::TurnInto(block) => {
            editing::apply_change(
                editor,
                EditOrigin::Command,
                window,
                cx,
                |document, selections, primary| set_block(document, selections, primary, *block),
            );
        }
    }
    let focus = editor.focus_handle_ref().clone();
    window.focus(&focus, cx);
}

pub(crate) fn menu_key(
    editor: &mut NoteEditor,
    key: MenuKey,
    window: &Window,
    cx: &mut Context<'_, NoteEditor>,
) -> bool {
    completion::menu_key(editor, key, window, cx)
}

fn bubble_visible(editor: &NoteEditor, window: &Window, cx: &App) -> bool {
    let surfaces = editor.surfaces();
    let session = editor.session();
    let primary = session.primary();
    let focused = editor.focus_handle_ref().contains_focused(window, cx);
    let editing_link = surfaces.bubble.read(cx).is_editing_link();
    session.selections().len() == 1
        && !primary.is_empty()
        && !editor.pointer().is_dragging()
        && (editing_link || (focused && !surfaces.bubble_suppressed))
        && !inside_code(session.document(), primary)
}

pub(crate) fn escape(
    editor: &mut NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    if block_drag::cancel(editor, window, cx) {
        return;
    }
    if completion::close(editor, cx) {
        return;
    }
    if bubble_visible(editor, window, cx) {
        editor.surfaces_mut().bubble_suppressed = true;
        cx.notify();
        return;
    }
    let session = editor.session();
    if session.selections().len() == 1
        && let Some(change) =
            leave_table(session.document(), session.selections(), Direction::Forward)
    {
        editor.apply(change, EditOrigin::Command, window, cx);
        return;
    }
    if editor.session().selections().len() > 1 {
        let primary = editor.session().primary();
        editor.session_mut().select(primary);
        editor.edited(Edited::Selection, window, cx);
        return;
    }
    if editor.surfaces().find.is_open() {
        find::close(editor, window, cx);
    }
}

pub(crate) fn open_find(
    editor: &mut NoteEditor,
    replace: bool,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    find::open(editor, replace, window, cx);
}

pub(crate) fn open_slash_menu(
    editor: &mut NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    completion::open_slash_menu(editor, window, cx);
}

pub(crate) fn open_context_menu_at_caret(
    editor: &mut NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let head = editor.session().primary().head;
    let position = editor
        .hits()
        .caret_bounds(head)
        .map_or_else(|| point(px(0.0), px(0.0)), |bounds| bounds.bottom_left());
    context_menu::open(editor, position, head, window, cx);
}

pub(crate) fn open_lightbox(
    editor: &mut NoteEditor,
    images: Vec<LightboxImage>,
    index: usize,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let lightbox = cx.new(|cx| ImageLightbox::new(images, index, window, cx));
    let subscription = cx.subscribe_in(
        &lightbox,
        window,
        |editor, _, _: &LightboxEvent, window, cx| {
            editor.surfaces_mut().lightbox = None;
            let focus = editor.focus_handle_ref().clone();
            window.focus(&focus, cx);
            cx.notify();
        },
    );
    lightbox.focus_handle(cx).focus(window, cx);
    editor.surfaces_mut().lightbox = Some((lightbox, subscription));
    cx.notify();
}

pub(crate) fn focus_above_body(
    editor: &NoteEditor,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) {
    let has_properties = editor.session().mode() == Mode::LivePreview
        && editor
            .session()
            .rows()
            .first()
            .is_some_and(|row| row.kind == RowKind::Properties);
    if has_properties {
        editor
            .surfaces()
            .properties
            .update(cx, |panel, cx| panel.focus_add(window, cx));
    } else {
        editor.title_row().focus(window, cx);
    }
}

pub(crate) fn properties_row(editor: &NoteEditor, _cx: &mut Context<'_, NoteEditor>) -> AnyElement {
    metrics::column(
        div()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(editor.surfaces().properties.clone()),
    )
    .into_any_element()
}

pub(crate) fn render_overlays(
    editor: &NoteEditor,
    body: Stateful<Div>,
    window: &Window,
    cx: &Context<'_, NoteEditor>,
) -> (Stateful<Div>, Vec<usize>) {
    let surfaces = editor.surfaces();
    let hits = editor.hits();
    let session = editor.session();
    let primary = session.primary();
    let mut unplaced = Vec::new();
    let mut body = body.children(surfaces.find.bar().cloned());
    if let Some(menu) = surfaces.completing.visible_menu() {
        match hits.caret_bounds(primary.head) {
            Some(caret) => {
                body = body.child(deferred(
                    anchored()
                        .position(caret.bottom_left())
                        .snap_to_window_with_margin(px(8.0))
                        .child(div().mt_1().child(menu)),
                ));
            }
            None => unplaced.push(primary.head),
        }
    }
    if bubble_visible(editor, window, cx) {
        match (
            hits.caret_bounds(primary.start()),
            hits.caret_bounds(primary.end()),
        ) {
            (Some(start), Some(end)) => {
                let left = start.left().min(end.left());
                let right = start.left().max(end.left());
                let top = start.top().min(end.top());
                body = body.child(deferred(
                    anchored()
                        .anchor(Anchor::BottomCenter)
                        .position(point(left + (right - left) / 2.0, top))
                        .snap_to_window_with_margin(px(8.0))
                        .child(div().mb_2().child(surfaces.bubble.clone())),
                ));
            }
            _ => unplaced.extend([primary.start(), primary.end()]),
        }
    }
    if let Some(menu) = &surfaces.context_menu {
        body = body.child(deferred(
            anchored()
                .position(menu.position())
                .snap_to_window_with_margin(px(8.0))
                .child(menu.menu().clone()),
        ));
    }
    if let Some((lightbox, _)) = &surfaces.lightbox {
        let size = window.viewport_size();
        body = body.child(
            deferred(
                anchored()
                    .position(point(px(0.0), px(0.0)))
                    .child(div().w(size.width).h(size.height).child(lightbox.clone())),
            )
            .with_priority(2),
        );
    }
    (body, unplaced)
}
