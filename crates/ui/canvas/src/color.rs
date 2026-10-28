use std::ops::Range;

use gpui_kit::{Hsla, Rgba};
use margin_theme::Palette;

use crate::model::INK_COLOR;

const NEAR_WHITE_LUMINANCE: f32 = 0.85;

fn parse_hex(color: &str) -> Option<Rgba> {
    let hex = color.trim().trim_start_matches('#');
    let full: String = match hex.len() {
        3 => hex.chars().flat_map(|digit| [digit, digit]).collect(),
        6 => hex.to_owned(),
        _ => return None,
    };
    let channel = |range: Range<usize>| {
        u8::from_str_radix(full.get(range)?, 16)
            .ok()
            .map(|value| f32::from(value) / 255.0)
    };
    Some(Rgba {
        r: channel(0..2)?,
        g: channel(2..4)?,
        b: channel(4..6)?,
        a: 1.0,
    })
}

pub(crate) fn resolve(color: &str, palette: &Palette) -> Hsla {
    if color == INK_COLOR {
        return palette.canvas_ink;
    }
    match parse_hex(color) {
        Some(rgba)
            if 0.0722_f32.mul_add(rgba.b, 0.2126_f32.mul_add(rgba.r, 0.7152 * rgba.g))
                <= NEAR_WHITE_LUMINANCE =>
        {
            rgba.into()
        }
        _ => palette.canvas_ink,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_white_and_unknown_colors_become_ink() {
        let palette = Palette::dark();
        assert_eq!(resolve("#fff", &palette), palette.canvas_ink);
        assert_eq!(resolve("#f8f8f8", &palette), palette.canvas_ink);
        assert_eq!(resolve("tomato", &palette), palette.canvas_ink);
        assert_ne!(resolve("#ef4444", &palette), palette.canvas_ink);
        assert_eq!(resolve("#000", &palette), resolve("#000000", &palette));
    }
}
