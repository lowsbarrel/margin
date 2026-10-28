use gpui_kit::Hsla;

use crate::primitives::{blue, cyan, green, hsl, hsl_alpha, neutral, orange, purple, red, yellow};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CalloutTone {
    Note,
    Abstract,
    Info,
    Todo,
    Tip,
    Success,
    Question,
    Warning,
    Failure,
    Danger,
    Bug,
    Example,
    Quote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyntaxTone {
    Keyword,
    String,
    Number,
    Function,
    Comment,
    Attribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HighlightTone {
    Yellow,
    Blue,
    Green,
    Purple,
    Red,
    Orange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_disabled: Hsla,
    pub text_inverse: Hsla,
    pub text_brand: Hsla,
    pub text_positive: Hsla,
    pub text_warning: Hsla,
    pub text_negative: Hsla,
    pub text_code: Hsla,
    pub surface: [Hsla; 4],
    pub bg_brand: Hsla,
    pub bg_brand_hover: Hsla,
    pub bg_positive: Hsla,
    pub bg_warning: Hsla,
    pub bg_negative: Hsla,
    pub bg_negative_hover: Hsla,
    pub bg_overlay: Hsla,
    pub border_secondary: Hsla,
    pub border_divider: Hsla,
    pub border_strong: Hsla,
    pub canvas_grid: Hsla,
    pub canvas_ink: Hsla,
    pub lightbox_backdrop: Hsla,
    pub lightbox_control: Hsla,
    pub lightbox_control_hover: Hsla,
    pub logo_mark: Hsla,
    pub syntax: [Hsla; 6],
    pub callouts: [Hsla; 13],
    pub highlights: [Hsla; 6],
}

impl Palette {
    #[must_use]
    pub fn light() -> Self {
        let brand = orange(70);
        Self {
            dark: false,
            text_primary: neutral(100),
            text_secondary: neutral(60),
            text_tertiary: neutral(40),
            text_disabled: neutral(30),
            text_inverse: neutral(0),
            text_brand: brand,
            text_positive: green(70),
            text_warning: yellow(70),
            text_negative: red(70),
            text_code: red(70),
            surface: [
                neutral(0),
                hsl(0.0, 0.0, 97.5),
                hsl(0.0, 0.0, 94.5),
                hsl(0.0, 0.0, 91.0),
            ],
            bg_brand: brand,
            bg_brand_hover: orange(60),
            bg_positive: green(70),
            bg_warning: yellow(60),
            bg_negative: red(70),
            bg_negative_hover: red(60),
            bg_overlay: hsl_alpha(0.0, 0.0, 8.0, 32.0),
            border_secondary: hsl_alpha(0.0, 0.0, 25.0, 8.0),
            border_divider: neutral(10),
            border_strong: hsl_alpha(0.0, 0.0, 25.0, 16.0),
            canvas_grid: hsl_alpha(0.0, 0.0, 25.0, 12.0),
            canvas_ink: neutral(95),
            lightbox_backdrop: hsl_alpha(0.0, 0.0, 0.0, 85.0),
            lightbox_control: hsl_alpha(0.0, 0.0, 100.0, 70.0),
            lightbox_control_hover: hsl_alpha(0.0, 0.0, 100.0, 10.0),
            logo_mark: orange(50),
            syntax: [
                purple(70),
                green(70),
                hsl(22.0, 90.0, 38.0),
                hsl(216.0, 100.0, 40.0),
                neutral(50),
                red(70),
            ],
            callouts: [
                blue(70),
                cyan(60),
                blue(70),
                blue(70),
                cyan(60),
                green(70),
                yellow(60),
                yellow(60),
                red(60),
                red(70),
                red(70),
                purple(70),
                neutral(50),
            ],
            highlights: highlights(),
        }
    }

    #[must_use]
    pub fn dark() -> Self {
        let brand = orange(50);
        Self {
            dark: true,
            text_primary: neutral(0),
            text_secondary: neutral(40),
            text_tertiary: neutral(60),
            text_disabled: neutral(70),
            text_inverse: neutral(100),
            text_brand: brand,
            text_positive: green(40),
            text_warning: yellow(30),
            text_negative: red(40),
            text_code: orange(30),
            surface: [
                neutral(100),
                hsl(0.0, 0.0, 10.5),
                hsl(0.0, 0.0, 13.5),
                hsl(0.0, 0.0, 17.0),
            ],
            bg_brand: brand,
            bg_brand_hover: orange(40),
            bg_positive: green(40),
            bg_warning: yellow(30),
            bg_negative: red(40),
            bg_negative_hover: red(30),
            bg_overlay: hsl_alpha(0.0, 0.0, 0.0, 56.0),
            border_secondary: hsl_alpha(0.0, 0.0, 100.0, 8.0),
            border_divider: neutral(80),
            border_strong: hsl_alpha(0.0, 0.0, 100.0, 16.0),
            canvas_grid: hsl_alpha(0.0, 0.0, 100.0, 14.0),
            canvas_ink: neutral(5),
            lightbox_backdrop: hsl_alpha(0.0, 0.0, 0.0, 85.0),
            lightbox_control: hsl_alpha(0.0, 0.0, 100.0, 70.0),
            lightbox_control_hover: hsl_alpha(0.0, 0.0, 100.0, 10.0),
            logo_mark: orange(50),
            syntax: [
                hsl(286.0, 60.0, 67.0),
                hsl(95.0, 38.0, 62.0),
                hsl(39.0, 67.0, 69.0),
                hsl(187.0, 47.0, 55.0),
                neutral(50),
                hsl(5.0, 74.0, 66.0),
            ],
            callouts: [
                blue(40),
                cyan(40),
                blue(40),
                blue(40),
                cyan(40),
                green(40),
                yellow(40),
                yellow(40),
                red(40),
                red(40),
                red(40),
                purple(40),
                neutral(40),
            ],
            highlights: highlights(),
        }
    }

    #[must_use]
    pub const fn surface(&self, level: usize) -> Hsla {
        match level {
            0 => self.surface[0],
            1 => self.surface[1],
            2 => self.surface[2],
            _ => self.surface[3],
        }
    }

    #[must_use]
    pub fn brand(&self, alpha_percent: f32) -> Hsla {
        self.bg_brand.opacity(alpha_percent / 100.0)
    }

    #[must_use]
    pub const fn syntax(&self, tone: SyntaxTone) -> Hsla {
        self.syntax[tone as usize]
    }

    #[must_use]
    pub const fn callout(&self, tone: CalloutTone) -> Hsla {
        self.callouts[tone as usize]
    }

    #[must_use]
    pub const fn highlight(&self, tone: HighlightTone) -> Hsla {
        self.highlights[tone as usize]
    }
}

fn highlights() -> [Hsla; 6] {
    [
        hsl_alpha(40.0, 96.0, 52.0, 45.0),
        hsl_alpha(216.0, 100.0, 56.0, 40.0),
        hsl_alpha(112.0, 45.0, 52.0, 40.0),
        hsl_alpha(275.0, 72.0, 58.0, 40.0),
        hsl_alpha(11.0, 92.0, 55.0, 38.0),
        hsl_alpha(15.0, 100.0, 60.0, 42.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_invert_text_and_surface() {
        let light = Palette::light();
        let dark = Palette::dark();
        assert!(
            light.text_primary.l < light.surface(0).l,
            "light text must be darker than its page"
        );
        assert!(
            dark.text_primary.l > dark.surface(0).l,
            "dark text must be lighter than its page"
        );
    }

    #[test]
    fn surfaces_step_away_from_the_page() {
        for palette in [Palette::light(), Palette::dark()] {
            let page = palette.surface(0).l;
            let distances: Vec<f32> = (0..4)
                .map(|level| (palette.surface(level).l - page).abs())
                .collect();
            assert!(
                distances.is_sorted_by(|lower, higher| lower < higher),
                "surfaces must order by elevation"
            );
        }
    }

    #[test]
    fn tones_map_to_distinct_slots() {
        let palette = Palette::light();
        assert_eq!(
            palette.callout(CalloutTone::Note),
            palette.callout(CalloutTone::Info)
        );
        assert_ne!(
            palette.callout(CalloutTone::Note),
            palette.callout(CalloutTone::Danger)
        );
        assert_ne!(
            palette.syntax(SyntaxTone::Keyword),
            palette.syntax(SyntaxTone::String)
        );
        assert_ne!(
            palette.highlight(HighlightTone::Yellow),
            palette.highlight(HighlightTone::Blue)
        );
    }
}
