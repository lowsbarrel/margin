use std::cell::Cell;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use gpui_kit::{
    AnyWindowHandle, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, Render, Subscription, Window,
};
use margin_breadcrumbs::Breadcrumbs;
use margin_editor_assets::{DiagramCache, VaultIndex};
use margin_kernel::RelativePath;
use margin_live_preview::Mode;
use margin_text_buffer::{Change, EditOrigin, Selection};
use margin_workbench::{CurrentVault, VaultServices, WorkbenchEvent};

use crate::block_drag::BlockDrag;
use crate::caret_blink::CaretBlink;
use crate::hit_map::HitMap;
use crate::pointer::PointerState;
use crate::render_state::RenderState;
use crate::save::SaveController;
use crate::session::Session;
use crate::surfaces::Surfaces;
use crate::title::TitleRow;
use crate::{anchor, export, keys, persistence, surfaces, view, words};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ViewMode {
    #[default]
    LivePreview,
    Raw,
}

impl ViewMode {
    const fn mode(self) -> Mode {
        match self {
            Self::LivePreview => Mode::LivePreview,
            Self::Raw => Mode::Raw,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edited {
    Text,
    Selection,
}

pub struct NoteEditor {
    path: RelativePath,
    services: Arc<VaultServices>,
    window: AnyWindowHandle,
    focus_handle: FocusHandle,
    session: Session,
    mode: ViewMode,
    loaded: bool,
    hits: HitMap,
    save: SaveController,
    title: TitleRow,
    crumbs: Entity<Breadcrumbs>,
    surfaces: Surfaces,
    pending_cursor: Option<usize>,
    pending_anchor: Option<String>,
    marked: Option<Range<usize>>,
    pointer: PointerState,
    block_drag: BlockDrag,
    render: RenderState,
    words: Cell<(u64, usize)>,
    _subscriptions: Vec<Subscription>,
}

impl fmt::Debug for NoteEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NoteEditor")
            .field("path", &self.path)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl EventEmitter<WorkbenchEvent> for NoteEditor {}

impl Focusable for NoteEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for NoteEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        DiagramCache::release_unused(window, cx);
        let focused = self.focus_handle.is_focused(window);
        CaretBlink::update(self, focused, window, cx);
        view::render(self, window, cx)
    }
}

impl NoteEditor {
    pub fn new(path: RelativePath, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        keys::bind_once(cx);
        if VaultIndex::get(cx).is_none_or(|index| !index.is_ready()) {
            VaultIndex::refresh(cx);
        }
        let services = CurrentVault::expect(cx);
        let crumbs = cx.new(|cx| Breadcrumbs::new(path.clone(), window, cx));
        let crumbs_events = cx.subscribe(&crumbs, |_, _, event: &WorkbenchEvent, cx| {
            cx.emit(event.clone());
        });
        let title = TitleRow::new(&path, window, cx);
        let focus_handle = cx.focus_handle();
        let surfaces = Surfaces::new(&focus_handle, window, cx);
        let editor = Self {
            path,
            services,
            window: window.window_handle(),
            focus_handle,
            session: Session::new("", Mode::LivePreview),
            mode: ViewMode::LivePreview,
            loaded: false,
            hits: HitMap::default(),
            save: SaveController::default(),
            title,
            crumbs,
            surfaces,
            pending_cursor: None,
            pending_anchor: None,
            marked: None,
            pointer: PointerState::default(),
            block_drag: BlockDrag::default(),
            render: RenderState::new(cx),
            words: Cell::new((u64::MAX, 0)),
            _subscriptions: vec![crumbs_events],
        };
        persistence::load(&editor, cx);
        editor
    }

    pub fn view_mode(&self) -> ViewMode {
        self.mode
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<'_, Self>) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.session.set_mode(mode.mode());
        self.session.reveal_primary();
        cx.notify();
    }

    pub fn toggle_view_mode(&mut self, cx: &mut Context<'_, Self>) {
        let next = match self.mode {
            ViewMode::LivePreview => ViewMode::Raw,
            ViewMode::Raw => ViewMode::LivePreview,
        };
        self.set_view_mode(next, cx);
    }

    pub fn scroll_to_anchor(
        &mut self,
        anchor: &str,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !self.loaded {
            self.pending_anchor = Some(anchor.to_owned());
            return;
        }
        if let Some(selection) = anchor::find(self.session.document(), anchor) {
            self.session.select(selection);
            self.reveal(selection.head);
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    pub fn cursor_offset(&self) -> usize {
        self.session.primary().head
    }

    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "the Shell calls every view method with the same window and context borrows"
    )]
    pub fn set_cursor_offset(
        &mut self,
        offset: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !self.loaded {
            self.pending_cursor = Some(offset);
            return;
        }
        let offset = offset.min(self.session.text().len());
        self.session.select(Selection::cursor(offset));
        self.reveal(offset);
        self.edited(Edited::Selection, window, cx);
    }

    pub fn cursor_line_column(&self) -> (usize, usize) {
        let head = self.session.primary().head;
        let document = self.session.document();
        let line = document.line_of(head);
        let start = document.line_span(line).start;
        let column = self
            .session
            .text()
            .get(start..head)
            .map_or(0, |prefix| prefix.chars().count());
        (line + 1, column + 1)
    }

    pub fn word_count(&self) -> usize {
        let revision = self.session.buffer().revision();
        let (cached_revision, cached) = self.words.get();
        if cached_revision == revision {
            return cached;
        }
        let count = words::count(self.session.document());
        self.words.set((revision, count));
        count
    }

    pub fn is_dirty(&self) -> bool {
        self.save.is_dirty(self.session.buffer().revision())
    }

    pub fn export_html(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) {
        export::export_html(self, cx);
    }

    pub(crate) const fn note_path(&self) -> &RelativePath {
        &self.path
    }

    pub(crate) fn services(&self) -> Arc<VaultServices> {
        Arc::clone(&self.services)
    }

    pub(crate) const fn session(&self) -> &Session {
        &self.session
    }

    pub(crate) fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    pub(crate) const fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub(crate) fn hits(&self) -> HitMap {
        self.hits.clone()
    }

    pub(crate) const fn render_state(&self) -> &RenderState {
        &self.render
    }

    pub(crate) const fn render_state_mut(&mut self) -> &mut RenderState {
        &mut self.render
    }

    pub(crate) const fn title_row(&self) -> &TitleRow {
        &self.title
    }

    pub(crate) fn title_row_mut(&mut self) -> &mut TitleRow {
        &mut self.title
    }

    pub(crate) const fn crumbs(&self) -> &Entity<Breadcrumbs> {
        &self.crumbs
    }

    pub(crate) const fn surfaces(&self) -> &Surfaces {
        &self.surfaces
    }

    pub(crate) fn surfaces_mut(&mut self) -> &mut Surfaces {
        &mut self.surfaces
    }

    pub(crate) const fn window_handle(&self) -> AnyWindowHandle {
        self.window
    }

    pub(crate) fn set_path(&mut self, path: RelativePath, cx: &mut Context<'_, Self>) {
        self.crumbs
            .update(cx, |crumbs, cx| crumbs.set_path(path.clone(), cx));
        self.path = path;
        cx.notify();
    }

    pub(crate) fn reveal(&self, offset: usize) {
        if let Some(item) = self.session.row_at_offset(offset) {
            self.session.list().scroll_to_reveal_item(item);
        }
    }

    pub(crate) fn apply(
        &mut self,
        change: Change,
        origin: EditOrigin,
        window: &Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        if !self.loaded || change.is_empty() {
            return false;
        }
        let changed = self.session.apply(change, origin).is_some();
        self.edited(
            if changed {
                Edited::Text
            } else {
                Edited::Selection
            },
            window,
            cx,
        );
        changed
    }

    pub(crate) fn edited(&mut self, kind: Edited, window: &Window, cx: &mut Context<'_, Self>) {
        if kind == Edited::Text {
            self.save.schedule(cx);
            self.block_drag = BlockDrag::default();
        }
        self.reveal(self.session.primary().head);
        self.render.caret_mut().restart();
        surfaces::after_edit(self, kind, window, cx);
        cx.notify();
    }

    pub(crate) fn flush_now(&mut self, cx: &Context<'_, Self>) {
        persistence::flush(self, cx);
    }

    pub(crate) fn reload_from_disk(&self, cx: &Context<'_, Self>) {
        persistence::reload(self, cx);
    }

    pub(crate) fn save_mut(&mut self) -> &mut SaveController {
        &mut self.save
    }

    pub(crate) const fn save_state(&self) -> &SaveController {
        &self.save
    }

    pub(crate) fn mark_loaded(&mut self, session: Session) -> (Option<usize>, Option<String>) {
        self.session = session;
        self.loaded = true;
        (self.pending_cursor.take(), self.pending_anchor.take())
    }

    pub(crate) const fn view_session_mode(&self) -> Mode {
        self.mode.mode()
    }

    pub(crate) fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub(crate) fn set_marked(&mut self, marked: Option<Range<usize>>) {
        self.marked = marked;
    }

    pub(crate) const fn pointer(&self) -> &PointerState {
        &self.pointer
    }

    pub(crate) fn pointer_mut(&mut self) -> &mut PointerState {
        &mut self.pointer
    }

    pub(crate) const fn block_drag(&self) -> &BlockDrag {
        &self.block_drag
    }

    pub(crate) fn block_drag_mut(&mut self) -> &mut BlockDrag {
        &mut self.block_drag
    }

    pub(crate) const fn focus_handle_ref(&self) -> &FocusHandle {
        &self.focus_handle
    }
}
