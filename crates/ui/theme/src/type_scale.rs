use gpui_kit::{Rems, Styled, rems};

const ROOT_PX: f32 = 14.0;

#[must_use]
pub fn css_px(pixels: f32) -> Rems {
    rems(pixels / ROOT_PX)
}

fn sized<T: Styled>(element: T, size_px: f32, line_px: f32) -> T {
    element
        .text_size(css_px(size_px))
        .line_height(css_px(line_px))
}

pub trait TextScale: Styled + Sized {
    #[must_use]
    fn text_ui_xs(self) -> Self {
        sized(self, 12.0, 16.0)
    }

    #[must_use]
    fn text_ui_sm(self) -> Self {
        sized(self, 14.0, 20.0)
    }

    #[must_use]
    fn text_ui_base(self) -> Self {
        sized(self, 16.0, 22.0)
    }

    #[must_use]
    fn text_ui_lg(self) -> Self {
        sized(self, 18.0, 26.0)
    }

    #[must_use]
    fn text_ui_xl(self) -> Self {
        sized(self, 20.0, 28.0)
    }

    #[must_use]
    fn text_ui_2xl(self) -> Self {
        sized(self, 24.0, 30.0)
    }
}

impl<T: Styled> TextScale for T {}
