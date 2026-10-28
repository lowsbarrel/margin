use gpui_kit::{DefiniteLength, Div, ParentElement, Pixels, Rems, Styled, div, px, relative, rems};

pub(crate) const COLUMN_WIDTH: Pixels = px(750.0);
pub(crate) const COLUMN_PADDING: Pixels = px(40.0);

const ROOT_PX: f32 = 14.0;
const BODY_REM: f32 = 15.0 / ROOT_PX;
const TITLE_REM: f32 = 32.0 / ROOT_PX;
const HEADING_SCALE: [f32; 6] = [1.625, 1.35, 1.15, 1.05, 1.0, 1.0];
const HEADING_SPACE_ABOVE: [f32; 6] = [1.5, 1.25, 1.0, 0.8, 0.6, 0.6];
const HEADING_SPACE_BELOW: f32 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TypeScale {
    Body,
    Heading(u8),
    Code,
    CodeLabel,
    Footnote,
    Title,
}

impl TypeScale {
    pub(crate) fn size(self) -> Rems {
        rems(self.size_rem())
    }

    fn size_rem(self) -> f32 {
        match self {
            Self::Body => BODY_REM,
            Self::Heading(level) => BODY_REM * heading_step(&HEADING_SCALE, level),
            Self::Code => BODY_REM * 0.875,
            Self::CodeLabel => BODY_REM * 0.875 * 0.85,
            Self::Footnote => BODY_REM * 0.75,
            Self::Title => TITLE_REM,
        }
    }

    pub(crate) fn line_height(self) -> DefiniteLength {
        relative(self.ratio())
    }

    pub(crate) fn line_rems(self) -> Rems {
        rems(self.size_rem() * self.ratio())
    }

    const fn ratio(self) -> f32 {
        match self {
            Self::Body | Self::Footnote => 1.6,
            Self::Code | Self::CodeLabel => 1.55,
            Self::Heading(_) => 1.3,
            Self::Title => 1.1875,
        }
    }
}

fn heading_step(steps: &[f32; 6], level: u8) -> f32 {
    let index = usize::from(level.clamp(1, 6) - 1);
    steps.get(index).copied().unwrap_or(1.0)
}

pub(crate) fn em(factor: f32) -> Rems {
    rems(BODY_REM * factor)
}

pub(crate) fn heading_space(level: u8) -> (Rems, Rems) {
    let size = TypeScale::Heading(level).size_rem();
    (
        rems(size * heading_step(&HEADING_SPACE_ABOVE, level)),
        rems(size * HEADING_SPACE_BELOW),
    )
}

pub(crate) fn list_step() -> Rems {
    em(1.5)
}

pub(crate) fn column(content: impl gpui_kit::IntoElement) -> Div {
    div().w_full().flex().justify_center().child(
        div()
            .w_full()
            .max_w(COLUMN_WIDTH)
            .px(COLUMN_PADDING)
            .child(content),
    )
}
