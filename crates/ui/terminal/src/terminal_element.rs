use std::panic;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    App, Bounds, Element, ElementId, ElementInputHandler, Entity, FocusHandle, Font,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Style, Window, font,
    relative,
};

use crate::colors::{Focus, TerminalColors};
use crate::grid_layout::GridLayout;
use crate::painter::{Frame, Painter};
use crate::session::Screen;
use crate::terminal_view::TerminalView;

pub(crate) struct TerminalElement {
    view: Entity<TerminalView>,
    screen: Arc<Screen>,
    focus_handle: FocusHandle,
    focus: Focus,
    cursor_visible: bool,
    colors: TerminalColors,
}

impl TerminalElement {
    pub(crate) const fn new(
        view: Entity<TerminalView>,
        screen: Arc<Screen>,
        focus_handle: FocusHandle,
        focus: Focus,
        cursor_visible: bool,
        colors: TerminalColors,
    ) -> Self {
        Self {
            view,
            screen,
            focus_handle,
            focus,
            cursor_visible,
            colors,
        }
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = Frame;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let base = font(cx.theme().mono_font_family.clone());
        let font_size = cx.theme().mono_font_size;
        let layout = measure(bounds, &base, font_size, window);
        self.view
            .update(cx, |view, cx| view.apply_layout(layout, cx));
        Painter::new(
            layout,
            &self.colors,
            base,
            font_size,
            self.focus,
            self.cursor_visible,
        )
        .frame(&self.screen, window)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        prepaint.paint(bounds, window, cx);
        window.handle_input(
            &self.focus_handle,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
    }
}

fn measure(bounds: Bounds<Pixels>, base: &Font, font_size: Pixels, window: &Window) -> GridLayout {
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(base);
    let cell_width = text_system
        .advance(font_id, font_size, 'm')
        .map_or(font_size, |advance| advance.width);
    let line_height = (text_system.ascent(font_id, font_size)
        + text_system.descent(font_id, font_size).abs())
    .ceil()
    .max(font_size);
    GridLayout::new(bounds, cell_width, line_height)
}
