use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};
use margin_editor_assets::{
    DiagramKind, DiagramState, Embed, EmbedSize, MathMode, VaultIndex, diagram_element,
    embed_image, file_card, math, resolve_embed, resolve_image,
};
use margin_live_preview::{Atom, AtomKind, ImageSize, LineView};

use crate::embed_card;
use crate::metrics::{self, COLUMN_PADDING, COLUMN_WIDTH, TypeScale};
use crate::row_context::{RowAction, RowContext};

const BASELINE_ABOVE_LINE_BOTTOM: f32 = 6.5;

pub(crate) fn render_atoms(
    view: &LineView,
    text: &str,
    item: usize,
    context: &RowContext,
    cx: &mut App,
) -> Vec<AnyElement> {
    view.atoms
        .iter()
        .enumerate()
        .map(|(index, atom)| render_atom(atom, text, item * 1000 + index, context, cx))
        .collect()
}

fn element_id(name: &'static str, id: usize) -> ElementId {
    ElementId::NamedInteger(name.into(), u64::try_from(id).unwrap_or(u64::MAX))
}

const fn embed_size(size: &ImageSize) -> EmbedSize {
    EmbedSize {
        width: size.width,
        height: size.height,
    }
}

fn clickable(
    element: impl IntoElement,
    id: ElementId,
    action: RowAction,
    context: &RowContext,
) -> AnyElement {
    let handler = context.handler();
    div()
        .id(id)
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| handler(action.clone(), window, cx))
        .child(element)
        .into_any_element()
}

fn render_atom(
    atom: &Atom,
    text: &str,
    id: usize,
    context: &RowContext,
    cx: &mut App,
) -> AnyElement {
    let palette = context.palette().clone();
    match &atom.kind {
        AtomKind::Embed(target) => render_embed(atom, &target.target, text, id, context, cx),
        AtomKind::Image { destination, alt } => {
            render_image(atom, destination, alt, id, context, cx)
        }
        AtomKind::Math { tex } => {
            let state = math(tex, MathMode::Inline, cx);
            let depth = match &state {
                DiagramState::Ready(diagram) => diagram.depth,
                _ => 0.0,
            };
            div()
                .flex_none()
                .px(metrics::em(0.1))
                .mb(px(BASELINE_ABOVE_LINE_BOTTOM - depth))
                .child(diagram_element(&state, DiagramKind::Math, tex, cx))
                .into_any_element()
        }
        AtomKind::Checkbox { checked } => {
            let handler = context.handler();
            let marker = atom.source;
            div()
                .flex()
                .flex_none()
                .items_center()
                .w(metrics::list_step())
                .h(TypeScale::Body.line_rems())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Checkbox::new(element_id("task", id))
                        .checked(*checked)
                        .on_click(move |_, window, cx| {
                            handler(RowAction::ToggleTask { marker }, window, cx);
                        }),
                )
                .into_any_element()
        }
        AtomKind::FootnoteReference { id: label, number } => div()
            .self_start()
            .px(metrics::em(0.1))
            .text_size(TypeScale::Footnote.size())
            .line_height(TypeScale::Footnote.line_height())
            .text_color(palette.text_brand)
            .child(clickable(
                SharedString::from(number.to_string()),
                element_id("footnote", id),
                RowAction::JumpToFootnote(label.clone()),
                context,
            ))
            .into_any_element(),
        AtomKind::FootnoteDefinition { id: label, number } => div()
            .flex_none()
            .min_w(metrics::list_step())
            .text_color(palette.text_secondary)
            .child(SharedString::from(number.map_or_else(
                || format!("{label}."),
                |number| format!("{number}."),
            )))
            .into_any_element(),
        AtomKind::LineBreak => div().w_full().into_any_element(),
    }
}

fn embed_inner(source: &str) -> String {
    source
        .trim_start_matches('!')
        .trim_start_matches("[[")
        .trim_end_matches("]]")
        .to_owned()
}

fn render_embed(
    atom: &Atom,
    target: &str,
    text: &str,
    id: usize,
    context: &RowContext,
    cx: &mut App,
) -> AnyElement {
    let palette = context.palette().clone();
    let max_width = COLUMN_WIDTH - COLUMN_PADDING * 2.0;
    let inner = embed_inner(
        text.get(atom.source.start..atom.source.end)
            .unwrap_or_default(),
    );
    let resolved =
        VaultIndex::get(cx).and_then(|index| resolve_embed(index, context.note(), &inner));
    match resolved {
        Some(Embed::Image(image)) => clickable(
            div().py(metrics::em(0.5)).child(embed_image(
                &image.location,
                image.size.or_else(|| atom.size.as_ref().map(embed_size)),
                max_width,
            )),
            element_id("embed-image", id),
            RowAction::OpenImage {
                source: atom.source,
            },
            context,
        ),
        Some(Embed::Note {
            name,
            heading,
            path,
        }) => embed_card::note_card(&name, heading.as_deref(), path.as_ref(), context, cx),
        Some(Embed::File { name, path }) => {
            let card = file_card(&name, path.as_ref(), cx);
            match path {
                Some(path) => clickable(
                    card,
                    element_id("embed-file", id),
                    RowAction::OpenPath(path),
                    context,
                ),
                None => card.into_any_element(),
            }
        }
        None => div()
            .text_color(palette.text_tertiary)
            .child(SharedString::from(target.to_owned()))
            .into_any_element(),
    }
}

fn render_image(
    atom: &Atom,
    destination: &str,
    alt: &str,
    id: usize,
    context: &RowContext,
    cx: &App,
) -> AnyElement {
    let palette = context.palette().clone();
    let max_width = COLUMN_WIDTH - COLUMN_PADDING * 2.0;
    let resolved = VaultIndex::get(cx)
        .and_then(|index| resolve_image(index, context.note(), destination, alt));
    match resolved {
        Some(image) => clickable(
            div().py(metrics::em(0.5)).child(embed_image(
                &image.location,
                image.size.or_else(|| atom.size.as_ref().map(embed_size)),
                max_width,
            )),
            element_id("image", id),
            RowAction::OpenImage {
                source: atom.source,
            },
            context,
        ),
        None => div()
            .text_color(palette.text_tertiary)
            .child(SharedString::from(alt.to_owned()))
            .into_any_element(),
    }
}
