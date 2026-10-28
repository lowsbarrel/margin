use gpui_kit::{App, AppContext, Context, Entity, EntityId, EventEmitter, Subscription, Window};
use margin_canvas::CanvasView;
use margin_editor::{NoteEditor, ViewMode};
use margin_kernel::RelativePath;
use margin_panes::PaneItem;
use margin_viewers::{ImageView, PdfView, UnknownFileView};
use margin_workbench::{FileKind, TabHandle, WorkbenchEvent};

use crate::workspace::Workspace;

#[derive(Clone)]
pub(crate) enum TabView {
    Note(Entity<NoteEditor>),
    Canvas(Entity<CanvasView>),
    Image(Entity<ImageView>),
    Pdf(Entity<PdfView>),
    File(Entity<UnknownFileView>),
}

impl TabView {
    pub(crate) fn open(path: RelativePath, window: &mut Window, cx: &mut App) -> Self {
        match FileKind::of(&path) {
            FileKind::Note => Self::Note(cx.new(|cx| NoteEditor::new(path, window, cx))),
            FileKind::Canvas => Self::Canvas(cx.new(|cx| CanvasView::new(path, window, cx))),
            FileKind::Image => Self::Image(cx.new(|cx| ImageView::new(path, window, cx))),
            FileKind::Pdf => Self::Pdf(cx.new(|cx| PdfView::new(path, window, cx))),
            FileKind::Other => Self::File(cx.new(|cx| UnknownFileView::new(path, window, cx))),
        }
    }

    pub(crate) fn subscribe(
        &self,
        window: &Window,
        cx: &mut Context<'_, Workspace>,
    ) -> Subscription {
        match self {
            Self::Note(view) => subscribe(view, window, cx),
            Self::Canvas(view) => subscribe(view, window, cx),
            Self::Image(view) => subscribe(view, window, cx),
            Self::Pdf(view) => subscribe(view, window, cx),
            Self::File(view) => subscribe(view, window, cx),
        }
    }

    pub(crate) fn entity_id(&self) -> EntityId {
        self.handle().id()
    }

    pub(crate) const fn note(&self) -> Option<&Entity<NoteEditor>> {
        match self {
            Self::Note(editor) => Some(editor),
            Self::Canvas(_) | Self::Image(_) | Self::Pdf(_) | Self::File(_) => None,
        }
    }

    pub(crate) fn is_dirty(&self, cx: &App) -> bool {
        match self {
            Self::Note(editor) => editor.read(cx).is_dirty(),
            Self::Canvas(canvas) => canvas.read(cx).is_dirty(),
            Self::Image(_) | Self::Pdf(_) | Self::File(_) => false,
        }
    }

    pub(crate) const fn type_name(&self) -> &'static str {
        match self {
            Self::Note(_) => "markdown",
            Self::Canvas(_) => "canvas",
            Self::Image(_) => "image",
            Self::Pdf(_) => "pdf",
            Self::File(_) => "unknown",
        }
    }
}

pub(crate) const fn view_mode_name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::LivePreview => "rich",
        ViewMode::Raw => "source",
    }
}

pub(crate) fn view_mode_from_name(name: &str) -> ViewMode {
    if name == "source" {
        ViewMode::Raw
    } else {
        ViewMode::LivePreview
    }
}

fn subscribe<T: EventEmitter<WorkbenchEvent>>(
    entity: &Entity<T>,
    window: &Window,
    cx: &mut Context<'_, Workspace>,
) -> Subscription {
    cx.subscribe_in(entity, window, |workspace, _, event, window, cx| {
        workspace.on_workbench_event(event.clone(), window, cx);
    })
}

impl PaneItem for TabView {
    fn handle(&self) -> &dyn TabHandle {
        match self {
            Self::Note(view) => view,
            Self::Canvas(view) => view,
            Self::Image(view) => view,
            Self::Pdf(view) => view,
            Self::File(view) => view,
        }
    }
}
