use gpui_kit::component::{ActiveTheme, h_flex};
use gpui_kit::{
    Context, FontWeight, IntoElement, ParentElement, Pixels, Point, Render, SharedString, Styled,
    Window, div, px, rems,
};
use margin_theme::{MarginTheme, TextScale};

const POINTER_GAP: f32 = 14.0;

#[derive(Debug)]
pub struct DragPreview {
    label: SharedString,
    extra: usize,
    cursor_offset: Point<Pixels>,
}

impl DragPreview {
    #[must_use]
    pub const fn new(label: SharedString, extra: usize, cursor_offset: Point<Pixels>) -> Self {
        Self {
            label,
            extra,
            cursor_offset,
        }
    }
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let palette = MarginTheme::palette(cx);
        let chip = h_flex()
            .gap_1p5()
            .px_2p5()
            .py(rems(0.3125))
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(palette.border_secondary)
            .bg(palette.surface(0))
            .shadow_lg()
            .text_ui_xs()
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .text_color(palette.text_primary)
            .child(self.label.clone())
            .children((self.extra > 0).then(|| {
                div()
                    .text_color(palette.text_tertiary)
                    .child(SharedString::from(format!("+{}", self.extra)))
            }));
        div()
            .pl(self.cursor_offset.x + px(POINTER_GAP))
            .pt(self.cursor_offset.y + px(POINTER_GAP))
            .child(chip)
    }
}
