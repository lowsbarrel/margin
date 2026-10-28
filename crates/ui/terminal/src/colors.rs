use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use gpui_kit::{Hsla, Rgba};
use margin_theme::{Palette, SyntaxTone};

const DIM_OPACITY: f32 = 0.66;
const SELECTION_FOCUSED_PERCENT: f32 = 32.0;
const SELECTION_UNFOCUSED_PERCENT: f32 = 16.0;
const CUBE_START: u8 = 16;
const GRAY_START: u8 = 232;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Focus {
    Focused,
    Unfocused,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalColors {
    ansi: [Hsla; 16],
    foreground: Hsla,
    background: Hsla,
    cursor: Hsla,
    selection_focused: Hsla,
    selection_unfocused: Hsla,
}

impl TerminalColors {
    pub(crate) fn new(palette: &Palette) -> Self {
        let blue = palette.syntax(SyntaxTone::Function);
        let magenta = palette.syntax(SyntaxTone::Keyword);
        let normal = [
            palette.text_tertiary,
            palette.text_negative,
            palette.text_positive,
            palette.text_warning,
            blue,
            magenta,
            blue,
            palette.text_secondary,
        ];
        let mut ansi = [palette.text_primary; 16];
        ansi[..8].copy_from_slice(&normal);
        ansi[8..15].copy_from_slice(&normal[..7]);
        Self {
            ansi,
            foreground: palette.text_primary,
            background: palette.surface(0),
            cursor: palette.text_brand,
            selection_focused: palette.brand(SELECTION_FOCUSED_PERCENT),
            selection_unfocused: palette.brand(SELECTION_UNFOCUSED_PERCENT),
        }
    }

    pub(crate) const fn selection(&self, focus: Focus) -> Hsla {
        match focus {
            Focus::Focused => self.selection_focused,
            Focus::Unfocused => self.selection_unfocused,
        }
    }

    pub(crate) fn queried(&self, index: usize, overrides: &Colors) -> Option<Rgb> {
        let color = match index {
            256 => Color::Named(NamedColor::Foreground),
            257 => Color::Named(NamedColor::Background),
            258 => Color::Named(NamedColor::Cursor),
            _ => Color::Indexed(u8::try_from(index).ok()?),
        };
        Some(to_rgb(self.resolve(color, overrides)))
    }

    pub(crate) fn resolve(&self, color: Color, overrides: &Colors) -> Hsla {
        match color {
            Color::Spec(rgb) => from_rgb(rgb),
            Color::Indexed(index) => {
                overrides[usize::from(index)].map_or_else(|| self.indexed(index), from_rgb)
            }
            Color::Named(name) => {
                overrides[name as usize].map_or_else(|| self.named(name), from_rgb)
            }
        }
    }

    fn indexed(&self, index: u8) -> Hsla {
        if index < CUBE_START {
            return self.ansi[usize::from(index)];
        }
        if index < GRAY_START {
            let cube = index - CUBE_START;
            return from_rgb(Rgb {
                r: cube_level(cube / 36),
                g: cube_level(cube / 6 % 6),
                b: cube_level(cube % 6),
            });
        }
        let level = 8 + (index - GRAY_START) * 10;
        from_rgb(Rgb {
            r: level,
            g: level,
            b: level,
        })
    }

    fn named(&self, name: NamedColor) -> Hsla {
        match name {
            NamedColor::Foreground | NamedColor::BrightForeground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimForeground => self.foreground.opacity(DIM_OPACITY),
            NamedColor::DimBlack
            | NamedColor::DimRed
            | NamedColor::DimGreen
            | NamedColor::DimYellow
            | NamedColor::DimBlue
            | NamedColor::DimMagenta
            | NamedColor::DimCyan
            | NamedColor::DimWhite => {
                let normal = name as usize - NamedColor::DimBlack as usize;
                self.ansi[normal].opacity(DIM_OPACITY)
            }
            ansi => self.ansi[ansi as usize],
        }
    }

    pub(crate) fn dim(color: Hsla) -> Hsla {
        color.opacity(DIM_OPACITY)
    }
}

const fn cube_level(step: u8) -> u8 {
    if step == 0 { 0 } else { 55 + step * 40 }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "each channel is clamped to 0..=1 before scaling to a byte"
)]
fn to_rgb(color: Hsla) -> Rgb {
    let rgba = Rgba::from(color);
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgb {
        r: byte(rgba.r),
        g: byte(rgba.g),
        b: byte(rgba.b),
    }
}

fn from_rgb(rgb: Rgb) -> Hsla {
    Rgba {
        r: f32::from(rgb.r) / 255.0,
        g: f32::from(rgb.g) / 255.0,
        b: f32::from(rgb.b) / 255.0,
        a: 1.0,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(r: u8, g: u8, b: u8) -> Hsla {
        from_rgb(Rgb { r, g, b })
    }

    #[test]
    fn ansi_colours_follow_the_theme_roles() {
        for palette in [Palette::light(), Palette::dark()] {
            let colors = TerminalColors::new(&palette);
            let none = Colors::default();
            assert_eq!(
                colors.resolve(Color::Named(NamedColor::Red), &none),
                palette.text_negative
            );
            assert_eq!(
                colors.resolve(Color::Indexed(2), &none),
                palette.text_positive
            );
            assert_eq!(
                colors.resolve(Color::Named(NamedColor::BrightWhite), &none),
                palette.text_primary
            );
            assert_eq!(
                colors.resolve(Color::Named(NamedColor::Background), &none),
                palette.surface(0)
            );
            assert_eq!(
                colors.resolve(Color::Named(NamedColor::BrightBlue), &none),
                palette.syntax(SyntaxTone::Function)
            );
        }
    }

    #[test]
    fn the_256_colour_cube_and_grey_ramp_are_xterm_values() {
        let colors = TerminalColors::new(&Palette::dark());
        let none = Colors::default();
        assert_eq!(colors.resolve(Color::Indexed(16), &none), spec(0, 0, 0));
        assert_eq!(colors.resolve(Color::Indexed(196), &none), spec(255, 0, 0));
        assert_eq!(
            colors.resolve(Color::Indexed(110), &none),
            spec(135, 175, 215)
        );
        assert_eq!(
            colors.resolve(Color::Indexed(231), &none),
            spec(255, 255, 255)
        );
        assert_eq!(colors.resolve(Color::Indexed(232), &none), spec(8, 8, 8));
        assert_eq!(
            colors.resolve(Color::Indexed(255), &none),
            spec(238, 238, 238)
        );
    }

    #[test]
    fn truecolour_and_osc_overrides_win() {
        let colors = TerminalColors::new(&Palette::light());
        let mut overrides = Colors::default();
        let custom = Rgb { r: 1, g: 2, b: 3 };
        assert_eq!(
            colors.resolve(Color::Spec(custom), &overrides),
            spec(1, 2, 3)
        );
        overrides[1] = Some(custom);
        assert_eq!(colors.resolve(Color::Indexed(1), &overrides), spec(1, 2, 3));
        assert_eq!(
            colors.resolve(Color::Named(NamedColor::Red), &overrides),
            spec(1, 2, 3)
        );
    }
}
