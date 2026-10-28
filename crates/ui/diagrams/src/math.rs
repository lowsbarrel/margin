use std::sync::Arc;

use ratex_layout::LayoutOptions;
use ratex_svg::{SvgColorSyntax, SvgOptions};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;

use crate::diagram_style::DiagramStyle;
use crate::error::Error;
use crate::svg::Svg;

const GLYPH_OVERHANG_EM: f64 = 0.1;
const RULE_STROKE_EM: f64 = 0.0375;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathMode {
    Inline,
    Display,
}

pub fn render_math(tex: &str, mode: MathMode, style: &DiagramStyle) -> Result<Svg, Error> {
    let color = Color::from_hex(&style.foreground).ok_or_else(|| Error::InvalidColor {
        value: style.foreground.clone(),
    })?;
    let nodes = ratex_parser::parse(tex).map_err(|error| Error::Parse {
        message: error.to_string(),
    })?;
    let math_style = match mode {
        MathMode::Inline => MathStyle::Text,
        MathMode::Display => MathStyle::Display,
    };
    let root = ratex_layout::layout(
        &nodes,
        &LayoutOptions::default()
            .with_style(math_style)
            .with_color(color),
    );
    ratex_layout::validate_layout(&root).map_err(|message| Error::TooLarge { message })?;
    let list = ratex_layout::to_display_list(&root);
    let font_size = f64::from(style.font_size);
    let padding = font_size * GLYPH_OVERHANG_EM;
    let options = SvgOptions {
        font_size,
        padding,
        stroke_width: font_size * RULE_STROKE_EM,
        embed_glyphs: true,
        font_dir: String::new(),
    };
    let svg = ratex_svg::render_to_svg_with_color_syntax(&list, &options, SvgColorSyntax::Rgb);
    let in_user_units_not_points = svg.replacen("pt\"", "\"", 2);
    Ok(Svg {
        bytes: Arc::from(in_user_units_not_points.into_bytes()),
        width: to_f32(list.width.mul_add(font_size, 2.0 * padding)),
        height: to_f32((list.height + list.depth).mul_add(font_size, 2.0 * padding)),
        depth: to_f32(list.depth.mul_add(font_size, padding)),
    })
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "diagram sizes are a few thousand units, well inside f32"
)]
fn to_f32(value: f64) -> f32 {
    value as f32
}
