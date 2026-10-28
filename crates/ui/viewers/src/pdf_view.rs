use std::ops::Range;
use std::sync::Arc;

use gpui_kit::component::input::InputState;
use gpui_kit::component::v_flex;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ListAlignment, ListOffset, ListState, ParentElement, Render,
    ScrollHandle, Styled, Subscription, Window, div, px,
};
use margin_breadcrumbs::Breadcrumbs;
use margin_i18n::t_with;
use margin_kernel::RelativePath;
use margin_theme::{MarginTheme, css_px};
use margin_workbench::{CurrentVault, FindInNote, TabContent, WorkbenchEvent, run_blocking};

use crate::pdf_chrome::{on_find_input, on_key, on_page_input, pages, toolbar};
use crate::pdf_document::PdfDocument;
use crate::pdf_find::PdfFind;
use crate::pdf_layout::{PAGE_PADDING, page_top, page_under_center};
use crate::pdf_pages::{PageCache, PageLook, page_element, render_task};
use crate::pdf_pointer::copy_text;
use crate::pdf_select::PdfSelection;
use crate::zoom::{PDF_ZOOM, fit_width};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Scale {
    FitWidth,
    Manual(f32),
}

#[derive(Debug)]
pub(crate) enum Document {
    Loading,
    Ready(Arc<PdfDocument>),
    Failed(String),
}

#[derive(Debug)]
#[expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "the chrome, find and pointer modules drive the view's page state"
)]
pub struct PdfView {
    path: RelativePath,
    crumbs: Entity<Breadcrumbs>,
    _crumbs_subscription: Subscription,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) document: Document,
    pub(crate) sizes: Vec<(f32, f32)>,
    scale: Scale,
    container_width: f32,
    pub(crate) list_state: ListState,
    h_scroll: ScrollHandle,
    pub(crate) visible: Range<usize>,
    pub(crate) current_page: usize,
    pub(crate) cache: PageCache,
    pub(crate) find: PdfFind,
    page_input: Entity<InputState>,
    page_editing: bool,
    pub(crate) selection: PdfSelection,
    revision: u64,
    _subscriptions: Vec<Subscription>,
}

impl PdfView {
    pub fn new(path: RelativePath, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let find_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(margin_i18n::t(cx, "pdf_find_placeholder"))
        });
        let page_input = cx.new(|cx| InputState::new(window, cx).default_value("1"));
        let subscriptions = vec![
            cx.subscribe_in(&find_input, window, on_find_input),
            cx.subscribe_in(&page_input, window, on_page_input),
        ];
        let list_state = ListState::new(0, ListAlignment::Top, px(500.));
        let entity = cx.entity().downgrade();
        list_state.set_scroll_handler(move |event, window, cx| {
            let visible = event.visible_range.clone();
            let entity = entity.clone();
            window.defer(cx, move |window, cx| {
                entity
                    .update(cx, |view, cx| view.set_visible(visible, window, cx))
                    .unwrap_or_default();
            });
        });
        let crumbs = cx.new(|cx| Breadcrumbs::new(path.clone(), window, cx));
        let crumbs_subscription = cx.subscribe(&crumbs, |_, _, event: &WorkbenchEvent, cx| {
            cx.emit(event.clone());
        });
        let mut view = Self {
            path,
            crumbs,
            _crumbs_subscription: crumbs_subscription,
            focus_handle: cx.focus_handle(),
            document: Document::Loading,
            sizes: Vec::new(),
            scale: Scale::FitWidth,
            container_width: 0.0,
            list_state,
            h_scroll: ScrollHandle::new(),
            visible: 0..0,
            current_page: 0,
            cache: PageCache::default(),
            find: PdfFind::new(find_input),
            page_input,
            page_editing: false,
            selection: PdfSelection::default(),
            revision: 0,
            _subscriptions: subscriptions,
        };
        view.load(cx);
        view
    }

    fn load(&mut self, cx: &Context<'_, Self>) {
        self.revision += 1;
        let revision = self.revision;
        let files = Arc::clone(&CurrentVault::expect(cx).files);
        let path = self.path.clone();
        let task = run_blocking(cx, move || match files.read_file(&path) {
            Ok(bytes) => PdfDocument::load(bytes).map_err(|error| format!("{error:?}")),
            Err(error) => Err(error.to_string()),
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.cache.clear(cx);
                view.find.forget_document();
                view.selection.clear();
                match result {
                    Ok(document) => {
                        view.sizes = document.page_sizes();
                        view.list_state.reset(view.sizes.len());
                        view.document = Document::Ready(Arc::new(document));
                    }
                    Err(error) => {
                        view.sizes.clear();
                        view.list_state.reset(0);
                        view.document = Document::Failed(error);
                    }
                }
                cx.notify();
            })
        })
        .detach();
    }

    pub(crate) fn current_scale(&self) -> f32 {
        match self.scale {
            Scale::FitWidth => self
                .sizes
                .first()
                .map_or(1.0, |(width, _)| fit_width(*width, self.container_width)),
            Scale::Manual(scale) => scale,
        }
    }

    pub(crate) fn zoom_to(&mut self, scale: f32, cx: &mut Context<'_, Self>) {
        self.scale = Scale::Manual(PDF_ZOOM.clamp(scale));
        self.list_state.remeasure();
        cx.notify();
    }

    pub(crate) fn fit(&mut self, cx: &mut Context<'_, Self>) {
        self.scale = Scale::FitWidth;
        self.list_state.remeasure();
        cx.notify();
    }

    pub(crate) const fn current_page(&self) -> usize {
        self.current_page
    }

    pub(crate) fn go_to_page(
        &mut self,
        page: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.sizes.is_empty() {
            return;
        }
        let page = page.min(self.sizes.len() - 1);
        self.list_state.scroll_to(ListOffset {
            item_ix: page,
            offset_in_item: px(page_top(page)),
        });
        self.current_page = page;
        self.sync_page_input(window, cx);
        cx.notify();
    }

    fn set_visible(
        &mut self,
        visible: Range<usize>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.visible = visible;
        let top = self.list_state.logical_scroll_top();
        let height = f32::from(self.list_state.viewport_bounds().size.height);
        let page = page_under_center(
            &self.sizes,
            self.current_scale(),
            (top.item_ix, f32::from(top.offset_in_item)),
            height,
        );
        if page != self.current_page {
            self.current_page = page;
            self.sync_page_input(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn sync_page_input(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.page_editing {
            return;
        }
        let value = (self.current_page + 1).to_string();
        self.page_input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    pub(crate) const fn set_page_editing(&mut self, editing: bool) {
        self.page_editing = editing;
    }

    pub(crate) fn page_count(&self) -> usize {
        self.sizes.len()
    }

    pub(crate) fn is_fit_width(&self) -> bool {
        self.scale == Scale::FitWidth
    }

    pub(crate) const fn page_input(&self) -> &Entity<InputState> {
        &self.page_input
    }

    pub(crate) const fn find(&self) -> &PdfFind {
        &self.find
    }

    pub(crate) fn copy_selection(&self, cx: &Context<'_, Self>) {
        if let (Document::Ready(document), Some(range)) = (&self.document, self.selection.range()) {
            copy_text(Arc::clone(document), range, cx);
        }
    }

    pub(crate) fn render_page(
        &mut self,
        index: usize,
        window: &Window,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
        let scale = self.current_scale();
        let pixel_scale = scale * window.scale_factor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "cache key"
        )]
        let key = (pixel_scale * 1000.0).round() as u32;
        if let Document::Ready(document) = &self.document
            && self.cache.claim(index, key)
        {
            let task = render_task(document, index, pixel_scale, cx);
            let revision = self.revision;
            cx.spawn(async move |this, cx| {
                let rendered = task.await;
                this.update(cx, |view, cx| {
                    let rendered = rendered.filter(|_| view.revision == revision);
                    let anchor = view.visible.start;
                    view.cache.store(index, key, rendered, anchor, cx);
                    cx.notify();
                })
            })
            .detach();
        }
        let palette = MarginTheme::palette(cx);
        let selection = self
            .cache
            .text(index)
            .and_then(|text| {
                self.selection
                    .on_page(index, text.len())
                    .map(|range| text.highlight(range))
            })
            .unwrap_or_default();
        let look = PageLook {
            size: self.sizes.get(index).copied().unwrap_or_default(),
            scale,
            image: self.cache.image(index),
            hit: self.find.hit(index),
            selection,
            selection_fill: palette.brand(if self.focus_handle.is_focused(window) {
                32.
            } else {
                16.
            }),
        };
        page_element(index, &look, palette)
    }

    pub(crate) fn content_width(&self) -> f32 {
        let widest = self
            .sizes
            .iter()
            .map(|(width, _)| *width)
            .fold(0.0, f32::max);
        widest
            .mul_add(self.current_scale(), PAGE_PADDING * 2.0)
            .max(self.container_width)
    }

    pub(crate) fn list_state(&self) -> ListState {
        self.list_state.clone()
    }

    pub(crate) fn h_scroll(&self) -> ScrollHandle {
        self.h_scroll.clone()
    }

    pub(crate) fn set_container_width(&mut self, width: f32, cx: &mut Context<'_, Self>) {
        if (self.container_width - width).abs() > 0.5 {
            self.container_width = width;
            if self.scale == Scale::FitWidth {
                self.list_state.remeasure();
            }
            cx.notify();
        }
    }
}

impl Render for PdfView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let root = v_flex()
            .key_context("PdfView")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(on_key))
            .on_action(cx.listener(|view, _: &FindInNote, window, cx| view.open_find(window, cx)))
            .size_full()
            .overflow_hidden()
            .bg(palette.surface[2])
            .child(self.crumbs.clone());
        match &self.document {
            Document::Failed(error) => root.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .text_center()
                    .text_size(css_px(13.0))
                    .text_color(palette.text_tertiary)
                    .child(t_with(cx, "pdf_load_failed", &[("error", error)])),
            ),
            Document::Loading | Document::Ready(_) => {
                root.child(pages(self, cx)).child(toolbar(self, cx))
            }
        }
    }
}

impl Focusable for PdfView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<WorkbenchEvent> for PdfView {}

impl TabContent for PdfView {
    fn path(&self) -> &RelativePath {
        &self.path
    }

    fn rename(&mut self, to: RelativePath, cx: &mut Context<'_, Self>) {
        self.crumbs
            .update(cx, |crumbs, cx| crumbs.set_path(to.clone(), cx));
        self.path = to;
        cx.notify();
    }

    fn flush(&mut self, _cx: &mut Context<'_, Self>) {}

    fn reload(&mut self, cx: &mut Context<'_, Self>) {
        self.load(cx);
    }
}
