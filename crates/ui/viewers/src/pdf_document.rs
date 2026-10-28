use std::fmt;

use hayro::hayro_interpret::font::{Glyph, GlyphRun};
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::hayro_syntax::page::Page;
use hayro::hayro_interpret::hayro_syntax::{LoadPdfError, Pdf};
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, TransformExt, interpret_page,
};
use hayro::kurbo::{BezPath, Point, Rect};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

use crate::pdf_text::{PageBox, PageText};

const GLYPH_DESCENT: f64 = -200.0;
const GLYPH_ASCENT: f64 = 800.0;
const FALLBACK_ADVANCE: f32 = 500.0;

#[derive(Debug)]
pub(crate) struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub(crate) struct PdfDocument {
    pdf: Pdf,
}

impl fmt::Debug for PdfDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PdfDocument")
            .field("pages", &self.page_count())
            .finish()
    }
}

impl PdfDocument {
    pub(crate) fn load(bytes: Vec<u8>) -> Result<Self, LoadPdfError> {
        Pdf::new(bytes).map(|pdf| Self { pdf })
    }

    #[must_use]
    pub(crate) fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    #[must_use]
    pub(crate) fn page_sizes(&self) -> Vec<(f32, f32)> {
        self.pdf
            .pages()
            .iter()
            .map(Page::render_dimensions)
            .collect()
    }

    #[must_use]
    pub(crate) fn render(&self, index: usize, scale: f32) -> Option<RenderedPage> {
        let page = self.pdf.pages().get(index)?;
        let pixmap = hayro::render(
            page,
            &RenderCache::new(),
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &PixmapSettings {
                x_scale: scale,
                y_scale: scale,
                bg_color: WHITE,
            },
        );
        Some(RenderedPage {
            width: u32::from(pixmap.width()),
            height: u32::from(pixmap.height()),
            rgba: pixmap.data_as_u8_slice().to_vec(),
        })
    }

    #[must_use]
    pub(crate) fn page_text(&self, index: usize) -> PageText {
        let Some(page) = self.pdf.pages().get(index) else {
            return PageText::default();
        };
        let (width, height) = page.render_dimensions();
        let interpreter_cache = InterpreterCache::new();
        let mut context = Context::new(
            page.initial_transform(true).to_kurbo(),
            Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
            &interpreter_cache,
            page.xref(),
            InterpreterSettings::default(),
        );
        let mut collector = TextCollector::default();
        interpret_page(page, &mut context, &mut collector);
        collector.text
    }
}

#[derive(Default)]
struct TextCollector {
    text: PageText,
}

impl<'a> Device<'a> for TextCollector {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}

    fn push_clip_path(&mut self, _: &ClipPath) {}

    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}

    fn draw_glyph_run(&mut self, glyph_run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, _: &DrawMode) {
        for glyph in glyph_run.glyphs() {
            let Some(unicode) = glyph.as_unicode() else {
                continue;
            };
            let advance = match &**glyph {
                Glyph::Outline(outline) => outline.advance_width(),
                Glyph::Type3(_) => None,
            }
            .filter(|advance| *advance > 0.0)
            .unwrap_or(FALLBACK_ADVANCE);
            let transform = props.transform * glyph.transform();
            let corners = [
                Point::new(0.0, GLYPH_DESCENT),
                Point::new(f64::from(advance), GLYPH_DESCENT),
                Point::new(0.0, GLYPH_ASCENT),
                Point::new(f64::from(advance), GLYPH_ASCENT),
            ]
            .map(|corner| transform * corner);
            let rect = corners
                .iter()
                .fold(Rect::from_points(corners[0], corners[0]), |rect, corner| {
                    rect.union_pt(*corner)
                });
            #[expect(
                clippy::cast_possible_truncation,
                reason = "page coordinates fit in f32"
            )]
            let bounds = PageBox {
                x: rect.x0 as f32,
                y: rect.y0 as f32,
                width: rect.width() as f32,
                height: rect.height() as f32,
            };
            match unicode {
                BfString::Char(character) => {
                    self.text.push(character, bounds);
                }
                BfString::String(text) => {
                    for character in text.chars() {
                        self.text.push(character, bounds);
                    }
                }
            }
        }
    }

    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}

    fn pop_clip(&mut self) {}

    fn pop_transparency_group(&mut self) {}
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    fn fixture() -> Vec<u8> {
        let content = "BT /F1 24 Tf 72 700 Td (Hello Margin) Tj ET";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        ];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            writeln!(pdf, "{} 0 obj\n{object}\nendobj", index + 1).expect("string write");
        }
        let xref = pdf.len();
        writeln!(pdf, "xref\n0 {}\n0000000000 65535 f ", objects.len() + 1).expect("string write");
        for offset in offsets {
            writeln!(pdf, "{offset:010} 00000 n ").expect("string write");
        }
        write!(
            pdf,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .expect("string write");
        pdf.into_bytes()
    }

    #[test]
    fn renders_and_finds_text_in_a_generated_pdf() {
        let document = PdfDocument::load(fixture()).expect("fixture parses");
        assert_eq!(document.page_count(), 1);
        let page = document.render(0, 0.5).expect("page renders");
        assert_eq!((page.width, page.height), (306, 396));
        assert!(
            page.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] < 128)
        );
        let text = document.page_text(0);
        assert_eq!(text.slice(0..text.len()), "Hello Margin");
        let found = text.find(0, "margin");
        assert_eq!(found.len(), 1);
        let bounds = found[0].boxes[0];
        assert!(
            bounds.x > 72.0 && bounds.y > 60.0 && bounds.y < 100.0,
            "{bounds:?}"
        );
    }
}
