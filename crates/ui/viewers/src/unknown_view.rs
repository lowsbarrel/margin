use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_flex, v_flex};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Subscription,
    Window, div, px, rems,
};
use margin_breadcrumbs::Breadcrumbs;
use margin_i18n::t;
use margin_kernel::{Metadata, RelativePath};
use margin_theme::{MarginTheme, TextScale, css_px};
use margin_workbench::{CurrentVault, TabContent, WorkbenchEvent, file_title, run_blocking};

use crate::external::{format_size, modified_label, open_external, reveal};

const OCTET_STREAM: &str = "application/octet-stream";

#[derive(Debug)]
pub struct UnknownFileView {
    path: RelativePath,
    crumbs: Entity<Breadcrumbs>,
    _crumbs_subscription: Subscription,
    focus_handle: FocusHandle,
    metadata: Option<Metadata>,
    revision: u64,
}

fn type_label(path: &RelativePath) -> String {
    path.extension()
        .filter(|extension| !extension.is_empty())
        .map_or_else(|| OCTET_STREAM.to_owned(), str::to_uppercase)
}

impl UnknownFileView {
    pub fn new(path: RelativePath, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let crumbs = cx.new(|cx| Breadcrumbs::new(path.clone(), window, cx));
        let crumbs_subscription = cx.subscribe(&crumbs, |_, _, event: &WorkbenchEvent, cx| {
            cx.emit(event.clone());
        });
        let mut view = Self {
            path,
            crumbs,
            _crumbs_subscription: crumbs_subscription,
            focus_handle: cx.focus_handle(),
            metadata: None,
            revision: 0,
        };
        view.load(cx);
        view
    }

    fn load(&mut self, cx: &Context<'_, Self>) {
        self.revision += 1;
        let revision = self.revision;
        let files = Arc::clone(&CurrentVault::expect(cx).files);
        let path = self.path.clone();
        let task = run_blocking(cx, move || files.metadata(&path).ok());
        cx.spawn(async move |this, cx| {
            let metadata = task.await;
            this.update(cx, |view, cx| {
                if view.revision == revision {
                    view.metadata = metadata;
                    cx.notify();
                }
            })
        })
        .detach();
    }

    fn details(&self, cx: &App) -> Vec<(SharedString, SharedString)> {
        let mut rows = vec![(t(cx, "file_info_type"), type_label(&self.path).into())];
        if let Some(metadata) = self.metadata {
            rows.push((t(cx, "file_info_size"), format_size(metadata.size, cx)));
            if metadata.modified.as_millis() > 0 {
                rows.push((
                    t(cx, "file_info_modified"),
                    modified_label(metadata.modified, cx),
                ));
            }
        }
        rows
    }

    fn render_actions(&self, cx: &App) -> impl IntoElement + use<> {
        let path = self.path.clone();
        let reveal_path = self.path.clone();
        let action = |id: &'static str, icon: IconName, label: SharedString| {
            Button::new(id)
                .small()
                .h(css_px(24.5))
                .px_2p5()
                .rounded(px(8.))
                .text_ui_xs()
                .icon(Icon::new(icon).size(css_px(13.0)))
                .label(label)
        };
        h_flex()
            .flex_wrap()
            .justify_center()
            .gap_2()
            .child(
                action(
                    "open-external",
                    IconName::ExternalLink,
                    t(cx, "viewer_open_external"),
                )
                .primary()
                .on_click(move |_, window, cx| open_external(&path, window, cx)),
            )
            .child(
                action(
                    "reveal",
                    IconName::FolderOpen,
                    t(cx, "viewer_reveal_in_finder"),
                )
                .on_click(move |_, window, cx| reveal(&reveal_path, window, cx)),
            )
    }
}

impl Render for UnknownFileView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let (labels, values): (Vec<_>, Vec<_>) = self.details(cx).into_iter().unzip();
        let column = |cells: Vec<SharedString>| {
            v_flex()
                .gap_1p5()
                .children(cells.into_iter().map(|cell| div().truncate().child(cell)))
        };
        let card = v_flex()
            .w_full()
            .max_w(rems(28.0))
            .items_center()
            .p_6()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(palette.border_secondary)
            .bg(palette.surface[0])
            .shadow_md()
            .text_center()
            .child(
                Icon::new(IconName::FileQuestionMark)
                    .size(css_px(30.0))
                    .mb_3()
                    .text_color(palette.text_tertiary),
            )
            .child(
                div()
                    .mb_5()
                    .max_w_full()
                    .truncate()
                    .text_ui_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.text_primary)
                    .child(file_title(&self.path).to_owned()),
            )
            .child(
                h_flex()
                    .mb_6()
                    .w_full()
                    .items_start()
                    .gap_5()
                    .text_left()
                    .text_ui_xs()
                    .child(column(labels).text_color(palette.text_tertiary))
                    .child(
                        column(values)
                            .flex_1()
                            .min_w_0()
                            .text_color(palette.text_primary),
                    ),
            )
            .child(self.render_actions(cx));
        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .child(self.crumbs.clone())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .bg(palette.surface[2])
                    .child(card),
            )
    }
}

impl Focusable for UnknownFileView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<WorkbenchEvent> for UnknownFileView {}

impl TabContent for UnknownFileView {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_is_the_upper_case_extension_or_the_generic_mime_type() {
        let label = |path: &str| type_label(&RelativePath::new(path).expect("valid path"));
        assert_eq!(label("a/data.bin"), "BIN");
        assert_eq!(label("archive.tar.gz"), "GZ");
        assert_eq!(label("Makefile"), OCTET_STREAM);
    }
}
