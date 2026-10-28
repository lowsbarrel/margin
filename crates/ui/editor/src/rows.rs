use std::ops::Range;
use std::rc::Rc;

use gpui_kit::component::input::Input;
use gpui_kit::{
    AnyElement, App, Context, ElementId, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled, Window, div,
};
use margin_editor_assets::{DiagramKind, MathMode, diagram_element, highlight_code, math, mermaid};
use margin_live_preview::{
    CodeLinePosition, LineView, Mode, Row, RowKind, SyntaxRole, render_line,
};
use margin_markdown::Document;
use margin_theme::MarginTheme;

use crate::atoms::render_atoms;
use crate::line_row::{LineParts, line_row};
use crate::metrics::{self, TypeScale};
use crate::note_editor::NoteEditor;
use crate::row_context::{RowAction, RowContext, Surface};
use crate::segment::{Overlay, OverlayColors};
use crate::session::HEADER_ROWS;
use crate::table_virtual::{TableViewport, VIRTUAL_TABLE_ROWS};
use crate::{row_actions, surfaces, table_row, table_virtual, title};

const DRAG_SOURCE_OPACITY: f32 = 0.4;

pub(crate) fn render_row(
    editor: &mut NoteEditor,
    item: usize,
    window: &mut Window,
    cx: &mut Context<'_, NoteEditor>,
) -> AnyElement {
    if item < HEADER_ROWS {
        return title_row(editor, cx);
    }
    let Some(row) = editor.session().row(item).cloned() else {
        return div().into_any_element();
    };
    let context = main_context(editor, window, cx);
    let element = match &row.kind {
        RowKind::Properties => surfaces::properties_row(editor, cx),
        RowKind::Table => {
            let viewport = table_viewport(editor, &row, item, window, cx);
            let document = editor.session().document();
            table_row::render(document, &row, item, &context, viewport.as_ref(), cx)
        }
        _ => document_row(editor.session().document(), &row, item, &context, cx),
    };
    if editor.block_drag().dims(row.span) {
        div()
            .opacity(DRAG_SOURCE_OPACITY)
            .child(element)
            .into_any_element()
    } else {
        element
    }
}

fn table_viewport(
    editor: &NoteEditor,
    row: &Row,
    item: usize,
    window: &Window,
    cx: &App,
) -> Option<TableViewport> {
    let document = editor.session().document();
    let table = document.block_at(row.span.start)?.table()?;
    if table.rows.len() <= VIRTUAL_TABLE_ROWS {
        return None;
    }
    let mut cache = editor.render_state().tables().borrow_mut();
    let visible = cache.visible(item, window);
    let widths = cache.columns(document, table, MarginTheme::palette(cx), window);
    Some(TableViewport::new(
        visible,
        table_virtual::row_height(window),
        widths,
    ))
}

fn main_context(editor: &NoteEditor, window: &Window, cx: &Context<'_, NoteEditor>) -> RowContext {
    let palette = MarginTheme::palette(cx).clone();
    let session = editor.session();
    let focused = editor.focus_handle_ref().is_focused(window);
    let selection = if focused {
        palette.brand(32.0)
    } else {
        palette.brand(16.0)
    };
    let colors = OverlayColors::new(
        palette.text_primary,
        selection,
        palette.brand(16.0),
        palette.brand(32.0),
    );
    let (matches, current) = editor.surfaces().find_highlights();
    let overlay = Rc::new(Overlay::new(
        session.selections().to_vec(),
        session.buffer().primary_index(),
        focused && editor.render_state().caret().is_visible(),
        matches,
        current,
        colors,
    ));
    let entity = cx.entity().downgrade();
    let handler = Rc::new(
        move |action: RowAction, window: &mut Window, cx: &mut App| {
            drop(entity.update(cx, |editor, cx| {
                row_actions::handle(editor, action, window, cx);
            }));
        },
    );
    RowContext::new(
        Surface::new(
            palette,
            session.mode(),
            overlay,
            editor.hits(),
            editor.render_state().images().clone(),
        ),
        handler,
        editor.note_path().clone(),
        Rc::new(session.folds().clone()),
        Rc::new(session.reveal().clone()),
    )
}

pub(crate) fn document_row(
    document: &Document,
    row: &Row,
    item: usize,
    context: &RowContext,
    cx: &mut App,
) -> AnyElement {
    match &row.kind {
        RowKind::Line | RowKind::Properties => {
            let view = render_line(document, row.lines.start, context.mode(), context.reveal());
            line(document, view, item, context, cx)
        }
        RowKind::Table => table_row::render(document, row, item, context, None, cx),
        RowKind::MathBlock { tex } => {
            let state = math(tex, MathMode::Display, cx);
            let element = diagram_element(&state, DiagramKind::Math, tex, cx);
            block(element, row.span.start, item, context)
        }
        RowKind::Mermaid { source } => {
            let state = mermaid(source, cx);
            let element = diagram_element(&state, DiagramKind::Mermaid, source, cx);
            block(element, row.span.start, item, context)
        }
    }
}

fn line(
    document: &Document,
    view: LineView,
    item: usize,
    context: &RowContext,
    cx: &mut App,
) -> AnyElement {
    let colors = code_colors(document, &view, context, cx);
    let atoms = render_atoms(&view, document.text(), item, context, cx);
    line_row(LineParts::new(view, item, colors, atoms), context, cx)
}

fn code_colors(
    document: &Document,
    view: &LineView,
    context: &RowContext,
    cx: &mut App,
) -> Vec<(Range<usize>, SyntaxRole)> {
    let Some(code) = &view.decor.code else {
        return Vec::new();
    };
    if code.position != CodeLinePosition::Body || context.mode() != Mode::LivePreview {
        return Vec::new();
    }
    let Some(language) = code.language.as_deref() else {
        return Vec::new();
    };
    let code_text = document
        .text()
        .get(code.content.start..code.content.end)
        .unwrap_or_default();
    let spans = highlight_code(language, code_text, cx);
    let map = &view.map;
    let mut colors = Vec::new();
    for (range, role) in spans.iter() {
        let start = code.content.start + range.start;
        let end = code.content.start + range.end;
        if end <= map.source_start() || start >= map.source_end() {
            continue;
        }
        let from = map.to_display(start.max(map.source_start()));
        let to = map.to_display(end.min(map.source_end()));
        if from < to {
            colors.push((from..to, *role));
        }
    }
    colors
}

fn block(element: AnyElement, start: usize, item: usize, context: &RowContext) -> AnyElement {
    let handler = context.handler();
    let embedded = context.is_embedded();
    let clickable = div()
        .id(ElementId::NamedInteger(
            "block".into(),
            u64::try_from(item).unwrap_or(u64::MAX),
        ))
        .w_full()
        .py_2()
        .flex()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| handler(RowAction::PlaceCaret(start), window, cx))
        .child(element);
    if embedded {
        clickable.into_any_element()
    } else {
        metrics::column(clickable).into_any_element()
    }
}

fn title_row(editor: &NoteEditor, cx: &Context<'_, NoteEditor>) -> AnyElement {
    let palette = MarginTheme::palette(cx);
    metrics::column(
        div()
            .pt(metrics::em(2.8))
            .pb(metrics::em(0.8))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(palette.text_primary)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(title::key_down))
            .child(
                Input::new(editor.title_row().input())
                    .appearance(false)
                    .p_0()
                    .h(TypeScale::Title.line_rems())
                    .text_size(TypeScale::Title.size())
                    .line_height(TypeScale::Title.line_height()),
            ),
    )
    .into_any_element()
}
