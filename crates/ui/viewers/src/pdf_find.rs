use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, h_flex};
use gpui_kit::{
    App, Context, Entity, IntoElement, ListOffset, ParentElement, SharedString, Styled, Task,
    Window, div, px,
};
use margin_i18n::{t, t_with};
use margin_theme::{MarginTheme, TextScale, css_px};
use margin_workbench::run_blocking;

use crate::controls::icon_button;
use crate::pdf_document::PdfDocument;
use crate::pdf_layout::centered_on;
use crate::pdf_text::{PageBox, PageText, PdfMatch};
use crate::pdf_view::{Document, PdfView};

type SearchResult = (Arc<[PageText]>, Vec<PdfMatch>);

#[derive(Debug)]
pub(crate) struct PdfFind {
    open: bool,
    input: Entity<InputState>,
    texts: Option<Arc<[PageText]>>,
    matches: Vec<PdfMatch>,
    index: usize,
    searching: bool,
    revision: u64,
    query: String,
}

impl PdfFind {
    pub(crate) const fn new(input: Entity<InputState>) -> Self {
        Self {
            open: false,
            input,
            texts: None,
            matches: Vec::new(),
            index: 0,
            searching: false,
            revision: 0,
            query: String::new(),
        }
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) const fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) const fn open(&mut self) {
        self.open = true;
    }

    pub(crate) const fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn forget_document(&mut self) {
        self.texts = None;
        self.revision += 1;
        self.searching = false;
        self.matches.clear();
        self.query.clear();
    }

    fn take_query(&mut self, query: &str) -> bool {
        if self.query == query {
            return false;
        }
        query.clone_into(&mut self.query);
        true
    }

    pub(crate) fn start(
        &mut self,
        query: String,
        document: &Arc<PdfDocument>,
        cx: &App,
    ) -> Option<(u64, Task<SearchResult>)> {
        if !self.take_query(&query) {
            return None;
        }
        self.revision += 1;
        self.matches.clear();
        self.index = 0;
        self.searching = !query.trim().is_empty();
        if !self.searching {
            return None;
        }
        let document = Arc::clone(document);
        let cached = self.texts.clone();
        let task = run_blocking(cx, move || {
            let texts = cached.unwrap_or_else(|| {
                (0..document.page_count())
                    .map(|page| document.page_text(page))
                    .collect()
            });
            let matches = texts
                .iter()
                .enumerate()
                .flat_map(|(page, text)| text.find(page, &query))
                .collect();
            (texts, matches)
        });
        Some((self.revision, task))
    }

    pub(crate) fn finish(&mut self, revision: u64, (texts, matches): SearchResult) -> bool {
        self.texts = Some(texts);
        if revision != self.revision {
            return false;
        }
        self.searching = false;
        self.matches = matches;
        self.index = 0;
        !self.matches.is_empty()
    }

    pub(crate) fn step(&mut self, step: isize) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        let count = self.matches.len().cast_signed();
        self.index = (self.index.cast_signed() + step)
            .rem_euclid(count)
            .cast_unsigned();
        true
    }

    pub(crate) fn current(&self) -> Option<&PdfMatch> {
        self.matches.get(self.index)
    }

    pub(crate) fn hit(&self, page: usize) -> &[PageBox] {
        self.current()
            .filter(|found| found.page == page)
            .map_or(&[], |found| found.boxes.as_slice())
    }

    pub(crate) fn status(&self, cx: &App) -> SharedString {
        if !self.matches.is_empty() {
            let current = (self.index + 1).to_string();
            let total = self.matches.len().to_string();
            return t_with(
                cx,
                "pdf_find_count",
                &[("current", &current), ("total", &total)],
            );
        }
        if self.input.read(cx).value().trim().is_empty() {
            SharedString::default()
        } else if self.searching {
            t(cx, "pdf_find_searching")
        } else {
            t(cx, "pdf_find_none")
        }
    }

    pub(crate) fn render(&self, cx: &Context<'_, PdfView>) -> impl IntoElement + use<> {
        let palette = MarginTheme::palette(cx);
        let tertiary = palette.text_tertiary;
        let empty = self.matches.is_empty();
        let placeholder = t(cx, "pdf_find_placeholder");
        h_flex()
            .absolute()
            .top_2()
            .right_3()
            .gap_1()
            .p_1()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(palette.border_secondary)
            .bg(palette.surface[0])
            .shadow_md()
            .child(
                Icon::new(IconName::Search)
                    .size(css_px(13.0))
                    .ml_1()
                    .flex_shrink_0()
                    .text_color(tertiary),
            )
            .child(
                div()
                    .w_40()
                    .child(Input::new(&self.input).text_ui_xs().aria_label(placeholder)),
            )
            .child(
                div()
                    .min_w_16()
                    .text_right()
                    .text_ui_xs()
                    .text_color(tertiary)
                    .child(self.status(cx)),
            )
            .child(
                icon_button(
                    "find-previous",
                    IconName::ChevronUp,
                    t(cx, "pdf_find_previous"),
                    tertiary,
                )
                .disabled(empty)
                .on_click(cx.listener(|view, _, window, cx| view.step_match(-1, window, cx))),
            )
            .child(
                icon_button(
                    "find-next",
                    IconName::ChevronDown,
                    t(cx, "pdf_find_next"),
                    tertiary,
                )
                .disabled(empty)
                .on_click(cx.listener(|view, _, window, cx| view.step_match(1, window, cx))),
            )
            .child(
                icon_button("find-close", IconName::X, t(cx, "find_close"), tertiary)
                    .on_click(cx.listener(|view, _, window, cx| view.close_find(window, cx))),
            )
    }
}

#[expect(
    clippy::multiple_inherent_impl,
    reason = "find navigation lives next to the find bar it drives"
)]
impl PdfView {
    pub(crate) fn open_find(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.find.open();
        self.find.input().update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn close_find(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.find.close();
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn search(&mut self, query: String, window: &Window, cx: &mut Context<'_, Self>) {
        cx.notify();
        let Document::Ready(document) = &self.document else {
            return;
        };
        let Some((revision, task)) = self.find.start(query, document, cx) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |view, window, cx| {
                if view.find.finish(revision, result) {
                    view.reveal_match(window, cx);
                }
                cx.notify();
            })
        })
        .detach();
    }

    pub(crate) fn step_match(
        &mut self,
        step: isize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.find.step(step) {
            self.reveal_match(window, cx);
        }
    }

    fn reveal_match(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(found) = self.find.current() else {
            return;
        };
        let page = found.page;
        let middle = found
            .boxes
            .first()
            .map_or(0.0, |bounds| bounds.height.mul_add(0.5, bounds.y));
        let height = f32::from(self.list_state.viewport_bounds().size.height);
        let (item_ix, offset) =
            centered_on(&self.sizes, self.current_scale(), page, middle, height);
        self.list_state.scroll_to(ListOffset {
            item_ix,
            offset_in_item: px(offset),
        });
        self.current_page = page;
        self.sync_page_input(window, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::init;
    use gpui_kit::{AppContext, TestAppContext};

    use super::*;

    fn found(page: usize) -> PdfMatch {
        PdfMatch {
            page,
            boxes: vec![PageBox {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }],
        }
    }

    #[gpui_kit::test]
    fn steps_wrap_and_only_the_current_match_is_highlighted(cx: &mut TestAppContext) {
        cx.update(init);
        let window = cx.add_empty_window();
        let input = window.update(|window, cx| cx.new(|cx| InputState::new(window, cx)));
        let mut find = PdfFind::new(input);
        assert!(find.finish(
            find.revision,
            (Arc::from([]), vec![found(0), found(0), found(2)])
        ));
        assert_eq!(find.hit(0).len(), 1);
        assert!(find.hit(2).is_empty());
        assert!(find.step(-1));
        assert_eq!(find.current().map(|found| found.page), Some(2));
        assert!(find.hit(0).is_empty() && find.hit(2).len() == 1);
        assert!(find.step(1));
        assert_eq!(find.current().map(|found| found.page), Some(0));
        find.close();
        assert!(!find.is_open() && find.current().is_some());
        let stale = find.revision;
        find.revision += 1;
        assert!(!find.finish(stale, (Arc::from([]), Vec::new())));
        assert!(find.current().is_some());
        assert!(find.take_query("vault"));
        assert!(!find.take_query("vault"));
        assert!(find.current().is_some());
        find.forget_document();
        assert!(find.take_query("vault"));
    }
}
