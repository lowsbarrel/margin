use std::ops::Range;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, Sizable, h_flex};
use gpui_kit::{
    AnyElement, Div, ElementId, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled,
    div, px, rems,
};
use margin_editor_assets::{LinkRef, Resolved, VaultIndex, resolve_link};
use margin_i18n::t;
use margin_kernel::RelativePath;
use margin_live_preview::{
    Atom, AtomKind, CodeLinePosition, LineView, LinkHit, LinkKind, ListDecor, ListMarker, Mode,
    OffsetMap, SyntaxRole,
};
use margin_markdown::Fold;

use crate::chips::chips_in;
use crate::frames::{callout_icon, callout_tone, code_frame, container_frame};
use crate::metrics::{self, TypeScale};
use crate::row_context::{RowAction, RowContext};
use crate::segment::{Styling, text_segment};
use crate::text_style::{Face, RunContext, code_runs, inline_runs};
use crate::word_chunks;

pub(crate) struct LineParts {
    view: LineView,
    item: usize,
    code_colors: Vec<(Range<usize>, SyntaxRole)>,
    atoms: Vec<AnyElement>,
}

impl LineParts {
    pub(crate) const fn new(
        view: LineView,
        item: usize,
        code_colors: Vec<(Range<usize>, SyntaxRole)>,
        atoms: Vec<AnyElement>,
    ) -> Self {
        Self {
            view,
            item,
            code_colors,
            atoms,
        }
    }
}

pub(crate) fn line_row(parts: LineParts, context: &RowContext, cx: &gpui_kit::App) -> AnyElement {
    let LineParts {
        view,
        item,
        code_colors,
        atoms,
    } = parts;
    let palette = context.palette();
    let code = view.decor.code.clone();
    let scale = if code.is_some() {
        TypeScale::Code
    } else {
        view.decor
            .heading
            .map_or(TypeScale::Body, TypeScale::Heading)
    };
    let face = if code.is_some() {
        Face::Mono
    } else {
        Face::Body
    };
    let color = match view.decor.heading {
        Some(5) => palette.text_secondary,
        Some(6) => palette.text_tertiary,
        _ if view.decor.comment || view.decor.footnote_definition.is_some() => {
            palette.text_tertiary
        }
        _ => palette.text_primary,
    };
    let mut inner = h_flex()
        .w_full()
        .text_size(scale.size())
        .line_height(scale.line_height());
    if let Some(level) = view.decor.heading {
        let (above, below) = metrics::heading_space(level);
        inner = inner.font_weight(FontWeight::SEMIBOLD).pt(above).pb(below);
    }
    let code_body = code
        .as_ref()
        .is_some_and(|code| code.position == CodeLinePosition::Body);
    let (leading, flow) = inline_pieces(
        &view,
        &Style {
            face,
            color,
            code_colors: code_body.then_some(code_colors.as_slice()),
        },
        atoms,
        context,
        cx,
    );
    let single = flow.len() == 1;
    let flow = flow.into_iter().map(|piece| {
        if single {
            div().w_full().child(piece).into_any_element()
        } else {
            piece
        }
    });
    inner = if leading.is_empty() {
        inner.flex_wrap().items_end().children(flow)
    } else {
        inner.items_start().children(leading).child(
            h_flex()
                .flex_1()
                .min_w_0()
                .flex_wrap()
                .items_end()
                .children(flow),
        )
    };
    let mut body = decorate(inner, &view, item, context, cx);
    if let Some(code) = &code {
        body = code_frame(body, code, item, context, cx);
    }
    for container in view.decor.containers.iter().rev() {
        body = container_frame(body, *container, palette);
    }
    if context.is_embedded() {
        body.into_any_element()
    } else {
        metrics::column(body).into_any_element()
    }
}

pub(crate) struct Style<'a> {
    face: Face,
    color: Hsla,
    code_colors: Option<&'a [(Range<usize>, SyntaxRole)]>,
}

impl Style<'_> {
    pub(crate) const fn prose(color: Hsla) -> Self {
        Self {
            face: Face::Body,
            color,
            code_colors: None,
        }
    }
}

pub(crate) fn inline_pieces(
    view: &LineView,
    style: &Style<'_>,
    atoms: Vec<AnyElement>,
    context: &RowContext,
    cx: &gpui_kit::App,
) -> (Vec<AnyElement>, Vec<AnyElement>) {
    let missing = |kind: &LinkKind| link_missing(kind, context.note(), cx);
    let run_context = RunContext {
        palette: context.palette(),
        face: style.face,
        color: style.color,
        missing: &missing,
        live: context.mode() == Mode::LivePreview,
    };
    let text_runs = |range: Range<usize>| match style.code_colors {
        Some(colors) => {
            let mut runs = code_runs(view.text.len(), colors, &run_context);
            slice_runs(&mut runs, range);
            runs
        }
        None => inline_runs(view, range, &run_context),
    };
    let map = Rc::new(view.map.clone());
    let links: Rc<[LinkHit]> = Rc::from(view.links.clone());
    let mut cursor = 0;
    let mut atoms = atoms.into_iter();
    let mut leading: Vec<AnyElement> = Vec::new();
    let mut pieces: Vec<AnyElement> = Vec::new();
    let wraps_by_word = view.atoms.iter().any(|atom| !is_leading(atom));
    let chips = |range: Range<usize>| {
        if style.code_colors.is_some() {
            Vec::new()
        } else {
            chips_in(view, range, context.palette(), run_context.live)
        }
    };
    let push_text = |pieces: &mut Vec<AnyElement>, range: Range<usize>| {
        let chunks = if wraps_by_word {
            word_chunks::word_chunks(&view.text, range)
        } else {
            vec![range]
        };
        for chunk in chunks {
            pieces.push(segment(
                &view.text,
                chunk.clone(),
                Styling::new(text_runs(chunk.clone()), chips(chunk)),
                (&map, &links),
                context,
            ));
        }
    };
    for atom in &view.atoms {
        let at = atom.display.min(view.text.len());
        if at > cursor {
            push_text(&mut pieces, cursor..at);
        }
        if let Some(element) = atoms.next() {
            if is_leading(atom) {
                leading.push(element);
            } else {
                pieces.push(element);
            }
        }
        cursor = at;
    }
    push_text(&mut pieces, cursor..view.text.len());
    (leading, pieces)
}

fn is_leading(atom: &Atom) -> bool {
    atom.display == 0
        && matches!(
            atom.kind,
            AtomKind::Checkbox { .. } | AtomKind::FootnoteDefinition { .. }
        )
}

fn segment(
    text: &str,
    range: Range<usize>,
    styling: Styling,
    (map, links): (&Rc<OffsetMap>, &Rc<[LinkHit]>),
    context: &RowContext,
) -> AnyElement {
    let slice = text.get(range.clone()).unwrap_or_default();
    text_segment(
        SharedString::from(slice.to_owned()),
        styling,
        Rc::clone(map),
        range,
        Rc::clone(links),
        context.overlay(),
        context.hits(),
    )
}

fn slice_runs(runs: &mut Vec<gpui_kit::TextRun>, range: Range<usize>) {
    let mut out = Vec::with_capacity(runs.len());
    let mut at = 0;
    for run in runs.drain(..) {
        let start = at.max(range.start);
        let end = (at + run.len).min(range.end);
        if start < end {
            let mut piece = run.clone();
            piece.len = end - start;
            out.push(piece);
        }
        at += run.len;
    }
    *runs = out;
}

fn decorate(
    inner: Div,
    view: &LineView,
    item: usize,
    context: &RowContext,
    cx: &gpui_kit::App,
) -> Div {
    let palette = context.palette();
    if view.decor.rule && view.text.is_empty() {
        return div()
            .w_full()
            .py(metrics::em(1.25))
            .child(div().w_full().h(px(1.0)).bg(palette.border_secondary));
    }
    if let Some(header) = &view.decor.callout_header {
        let accent = palette.callout(callout_tone(header.kind));
        let handler = context.handler();
        let block_start = header.block.start;
        let fold = header.fold;
        let mut row = h_flex()
            .w_full()
            .gap_1p5()
            .items_center()
            .text_size(TypeScale::Body.size())
            .line_height(TypeScale::Body.line_height())
            .text_color(accent)
            .font_weight(FontWeight::SEMIBOLD)
            .child(
                Icon::new(callout_icon(header.kind))
                    .size(metrics::em(1.05))
                    .text_color(accent),
            )
            .child(SharedString::from(header.title.clone()));
        if header.fold != Fold::None {
            let collapsed = context.is_collapsed(block_start, header.fold);
            let label = if collapsed {
                t(cx, "callout_expand")
            } else {
                t(cx, "callout_collapse")
            };
            row = row.child(
                Button::new(ElementId::NamedInteger("callout-fold".into(), item as u64))
                    .ghost()
                    .xsmall()
                    .text_color(accent)
                    .icon(Icon::new(if collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    }))
                    .tooltip(label)
                    .on_click(move |_, window, cx| {
                        handler(RowAction::ToggleFold { block_start, fold }, window, cx);
                    }),
            );
        }
        return row.child(inner.flex_1());
    }
    if let Some(list) = &view.decor.list {
        return list_frame(inner, list, view, context);
    }
    inner
}

fn list_frame(inner: Div, list: &ListDecor, view: &LineView, context: &RowContext) -> Div {
    let palette = context.palette();
    let step = metrics::list_step();
    let depth = u16::try_from(list.depth).unwrap_or(u16::MAX);
    let marker_box = || {
        div()
            .flex()
            .flex_none()
            .items_center()
            .w(step)
            .h(TypeScale::Body.line_rems())
            .text_color(palette.text_secondary)
    };
    let gutter = match &list.marker {
        Some(ListMarker::Bullet) => Some(
            marker_box()
                .pl(metrics::em(0.35))
                .text_size(metrics::em(1.15))
                .child("\u{2022}"),
        ),
        Some(ListMarker::Ordered(label)) => Some(
            marker_box()
                .justify_end()
                .pr(metrics::em(0.4))
                .child(SharedString::from(label.clone())),
        ),
        Some(ListMarker::Task(_)) => None,
        None => {
            let on_line = list.marker_source.start >= view.map.source_start()
                && list.marker_source.start <= view.map.source_end();
            (!on_line).then(marker_box)
        }
    };
    let guides = (0..depth).map(|level| {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(rems(step.0.mul_add(f32::from(level), metrics::em(0.55).0)))
            .w(px(1.0))
            .bg(palette.border_secondary)
    });
    h_flex()
        .relative()
        .w_full()
        .items_start()
        .pl(rems(step.0 * f32::from(depth)))
        .children(guides)
        .children(gutter)
        .child(inner.flex_1())
}

fn link_missing(kind: &LinkKind, note: &RelativePath, cx: &gpui_kit::App) -> bool {
    let LinkKind::Wiki(link) = kind else {
        return false;
    };
    if link.target.is_empty() {
        return false;
    }
    VaultIndex::get(cx).is_some_and(|index| {
        index.is_ready()
            && matches!(
                resolve_link(index, note, LinkRef::Wiki(link)),
                Resolved::MissingNote { .. }
            )
    })
}
