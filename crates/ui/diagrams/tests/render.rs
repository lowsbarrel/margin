use std::str;

use gpui_kit::Hsla;
use margin_diagrams::{DiagramStyle, Error, MathMode, Svg, render_math, render_mermaid};
use margin_theme::Palette;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

fn hex(color: Hsla) -> String {
    format!("#{:06x}", u32::from(color.to_rgb()) >> 8)
}

fn style() -> DiagramStyle {
    let palette = Palette::light();
    DiagramStyle {
        foreground: hex(palette.text_primary),
        background: hex(palette.surface(0)),
        accent: hex(palette.bg_brand),
        muted: hex(palette.text_tertiary),
        font_size: 16.0,
        dark: palette.dark,
    }
}

fn text(svg: &Svg) -> &str {
    str::from_utf8(&svg.bytes).expect("SVG is UTF-8")
}

fn rasterize_without_any_font(svg: &Svg) -> Pixmap {
    let tree = Tree::from_data(&svg.bytes, &Options::default()).expect("SVG parses");
    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height()).expect("non-empty pixmap");
    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
    pixmap
}

fn painted_pixels_without_any_font(svg: &Svg) -> usize {
    rasterize_without_any_font(svg)
        .pixels()
        .iter()
        .filter(|pixel| pixel.alpha() > 0)
        .count()
}

#[test]
fn fraction_sum_and_matrix_render_as_glyph_paths_with_sane_size() {
    let svg = render_math(
        r"\frac{a}{b} + \sum_{i=0}^{n} x_i \begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}",
        MathMode::Inline,
        &style(),
    )
    .expect("valid TeX renders");

    assert!(
        svg.width > 100.0 && svg.width < 400.0,
        "width {}",
        svg.width
    );
    assert!(
        svg.height > 20.0 && svg.height < 100.0,
        "height {}",
        svg.height
    );
    assert!(!text(&svg).contains("<text"));
    assert!(painted_pixels_without_any_font(&svg) > 100);
}

#[test]
fn fraction_reaches_below_the_baseline() {
    let svg = render_math(r"\frac{a}{b}", MathMode::Inline, &style()).expect("renders");

    assert!(svg.depth > 0.0 && svg.depth < svg.height);
}

#[test]
fn display_math_is_larger_than_inline() {
    let tex = r"\sum_{i=0}^{n} \frac{1}{i}";
    let inline = render_math(tex, MathMode::Inline, &style()).expect("inline renders");
    let display = render_math(tex, MathMode::Display, &style()).expect("display renders");

    assert!(display.height > inline.height);
}

#[test]
fn math_is_painted_in_the_style_foreground() {
    let foreground = u32::from(Palette::light().text_primary.to_rgb());
    let svg = render_math("x^2", MathMode::Inline, &style()).expect("renders");
    let pixmap = rasterize_without_any_font(&svg);
    let opaque: Vec<u32> = pixmap
        .pixels()
        .iter()
        .filter(|pixel| pixel.alpha() == u8::MAX)
        .map(|pixel| u32::from_be_bytes([pixel.red(), pixel.green(), pixel.blue(), u8::MAX]))
        .collect();

    assert!(!opaque.is_empty());
    assert!(opaque.iter().all(|&pixel| pixel == foreground));
}

#[test]
fn invalid_tex_reports_the_parse_message() {
    let error = render_math(r"\frac{a", MathMode::Display, &style()).expect_err("unclosed group");

    assert!(matches!(error, Error::Parse { .. }));
    assert!(!error.to_string().is_empty());
}

#[test]
fn flowchart_renders_in_style_colours() {
    let style = style();
    let svg = render_mermaid(
        "flowchart LR\n  A[Start] --> B{Ok?}\n  B -->|yes| C[Done]",
        &style,
    )
    .expect("flowchart renders");

    assert!(svg.width > 0.0 && svg.height > 0.0);
    assert!(text(&svg).contains(&style.foreground));
    assert!(text(&svg).contains(&style.accent));
    assert!(painted_pixels_without_any_font(&svg) > 0);
}

#[test]
fn sequence_diagram_renders() {
    let style = style();
    let svg = render_mermaid(
        "sequenceDiagram\n  Alice->>Bob: Hi\n  Bob-->>Alice: Hello",
        &style,
    )
    .expect("sequence renders");

    assert!(svg.width > 0.0 && svg.height > 0.0);
    assert!(text(&svg).contains(&style.background));
}

#[test]
fn every_diagram_kind_the_editor_offers_renders() {
    let sources = [
        "classDiagram\n  Animal <|-- Duck",
        "stateDiagram-v2\n  [*] --> Still\n  Still --> [*]",
        "erDiagram\n  CUSTOMER ||--o{ ORDER : places",
        "gantt\n  title Plan\n  dateFormat YYYY-MM-DD\n  section Work\n  Draft :a1, 2024-01-01, 3d",
        "pie title Pets\n  \"Dogs\" : 3\n  \"Cats\" : 2",
    ];
    for source in sources {
        let svg = render_mermaid(source, &style()).expect(source);
        assert!(svg.width > 0.0 && svg.height > 0.0, "{source}");
    }
}

#[test]
fn invalid_mermaid_reports_the_parse_message() {
    let error = render_mermaid("flowchart LR\n  A -->", &style()).expect_err("dangling edge");

    assert!(matches!(error, Error::Parse { .. }));
    assert!(!error.to_string().is_empty());
}
