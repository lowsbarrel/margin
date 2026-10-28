use std::sync::Arc;

use mermaid_rs_renderer::{LayoutConfig, Theme};

use crate::diagram_style::DiagramStyle;
use crate::error::Error;
use crate::svg::Svg;

pub fn render_mermaid(source: &str, style: &DiagramStyle) -> Result<Svg, Error> {
    let parsed =
        mermaid_rs_renderer::parse_mermaid_strict(source).map_err(|error| Error::Parse {
            message: error.to_string(),
        })?;
    let theme = theme(style);
    let config = LayoutConfig::default();
    let layout = mermaid_rs_renderer::compute_layout(&parsed.graph, &theme, &config);
    let dimensions = mermaid_rs_renderer::measure_svg_dimensions(&layout, &config, None);
    let svg = mermaid_rs_renderer::render_svg(&layout, &theme, &config);
    Ok(Svg {
        bytes: Arc::from(svg.into_bytes()),
        width: dimensions.viewbox_width,
        height: dimensions.viewbox_height,
        depth: 0.0,
    })
}

fn theme(style: &DiagramStyle) -> Theme {
    let base = if style.dark {
        Theme::dark()
    } else {
        Theme::mermaid_default()
    };
    let foreground = &style.foreground;
    let background = &style.background;
    let accent = &style.accent;
    let muted = &style.muted;
    Theme {
        font_family: Theme::modern().font_family,
        font_size: style.font_size,
        primary_color: background.clone(),
        primary_text_color: foreground.clone(),
        primary_border_color: accent.clone(),
        line_color: muted.clone(),
        edge_label_background: background.clone(),
        cluster_background: background.clone(),
        cluster_border: muted.clone(),
        background: background.clone(),
        sequence_actor_fill: background.clone(),
        sequence_actor_border: accent.clone(),
        sequence_actor_line: muted.clone(),
        sequence_activation_fill: background.clone(),
        sequence_activation_border: muted.clone(),
        text_color: foreground.clone(),
        pie_title_text_color: foreground.clone(),
        pie_section_text_color: foreground.clone(),
        pie_legend_text_color: foreground.clone(),
        pie_stroke_color: background.clone(),
        pie_outer_stroke_color: muted.clone(),
        ..base
    }
}
