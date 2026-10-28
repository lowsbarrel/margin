use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use margin_editor_assets::{
    EmbedKind, MAX_EMBED_DEPTH, NoteTextState, embed_icon, heading_section, note_text,
};
use margin_i18n::{t, t_with};
use margin_kernel::RelativePath;
use margin_live_preview::{CalloutFolds, Mode, Reveal, plan_rows};
use margin_markdown::Document;
use margin_text_buffer::Selection;

use crate::metrics;
use crate::row_context::{RowAction, RowContext};
use crate::rows::document_row;
use crate::segment::{Overlay, OverlayColors};

pub(crate) fn note_card(
    name: &str,
    heading: Option<&str>,
    path: Option<&RelativePath>,
    context: &RowContext,
    cx: &mut App,
) -> AnyElement {
    let palette = context.palette().clone();
    let title = heading.map_or_else(|| name.to_owned(), |heading| format!("{name} > {heading}"));
    let header = h_flex()
        .w_full()
        .gap(metrics::em(0.45))
        .items_center()
        .px(metrics::em(0.8))
        .py(metrics::em(0.4))
        .border_b_1()
        .border_color(palette.border_secondary)
        .text_size(metrics::em(0.78))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.text_brand)
        .child(embed_icon(EmbedKind::Note).size(metrics::em(0.78)))
        .child(SharedString::from(title.clone()));
    let body: AnyElement = match path {
        None => message(
            t_with(cx, "note_embed_missing", &[("title", name)]),
            context,
        ),
        Some(path) if context.depth() >= MAX_EMBED_DEPTH || path == context.note() => {
            message(SharedString::from(title), context)
        }
        Some(path) => match note_text(path, cx) {
            NoteTextState::Pending => div().into_any_element(),
            NoteTextState::Failed => message(
                t_with(cx, "note_embed_load_failed", &[("title", name)]),
                context,
            ),
            NoteTextState::Ready(text) => {
                let section = heading.map_or(&*text, |heading| heading_section(&text, heading));
                if section.trim().is_empty() {
                    message(t(cx, "note_embed_empty"), context)
                } else {
                    nested(section, path, context, cx)
                }
            }
        },
    };
    let header = match path.cloned() {
        Some(path) => {
            let handler = context.handler();
            header
                .id(ElementId::Name(SharedString::from(format!(
                    "embed-{}",
                    path.as_str()
                ))))
                .cursor_pointer()
                .hover(|style| style.bg(palette.surface(3)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, window, cx| {
                    handler(RowAction::OpenPath(path.clone()), window, cx);
                })
                .into_any_element()
        }
        None => header.into_any_element(),
    };
    let card = v_flex()
        .w_full()
        .rounded_r_xl()
        .border_t_1()
        .border_r_1()
        .border_b_1()
        .border_color(palette.border_secondary)
        .bg(palette.surface(1))
        .overflow_hidden()
        .child(header)
        .child(
            div()
                .px(metrics::em(0.85))
                .pt(metrics::em(0.35))
                .pb(metrics::em(0.5))
                .text_size(metrics::em(0.95))
                .child(body),
        );
    div()
        .w_full()
        .py(metrics::em(0.5))
        .child(
            div()
                .w_full()
                .pl(px(3.0))
                .rounded_xl()
                .bg(palette.bg_brand)
                .child(card),
        )
        .into_any_element()
}

fn message(text: SharedString, context: &RowContext) -> AnyElement {
    div()
        .py(metrics::em(0.5))
        .text_size(metrics::em(0.8))
        .text_color(context.palette().text_tertiary)
        .child(text)
        .into_any_element()
}

fn nested(text: &str, path: &RelativePath, context: &RowContext, cx: &mut App) -> AnyElement {
    let document = Document::parse(text);
    let palette = context.palette();
    let colors = OverlayColors::new(
        palette.text_primary,
        palette.brand(16.0),
        palette.brand(16.0),
        palette.brand(32.0),
    );
    let overlay = Rc::new(Overlay::new(
        Vec::<Selection>::new(),
        0,
        false,
        Vec::new(),
        None,
        colors,
    ));
    let inner = context.embedded(path.clone(), overlay);
    let rows = plan_rows(
        &document,
        Mode::LivePreview,
        &Reveal::default(),
        &CalloutFolds::default(),
    );
    let mut column = v_flex().w_full();
    for (index, row) in rows.iter().enumerate() {
        column = column.child(document_row(&document, row, index, &inner, cx));
    }
    column.into_any_element()
}
