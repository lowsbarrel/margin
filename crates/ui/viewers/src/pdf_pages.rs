use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::{
    AnyElement, App, Hsla, ImageSource, IntoElement, ParentElement, RenderImage, Styled, Task, div,
    img, px,
};
use margin_theme::Palette;
use margin_workbench::run_blocking;

use crate::pdf_document::PdfDocument;
use crate::pdf_layout::page_top;
use crate::pdf_text::{PageBox, PageText};

const CACHED_PAGES: usize = 12;

#[derive(Debug)]
pub(crate) struct RenderedPage {
    image: Arc<RenderImage>,
    text: Arc<PageText>,
}

#[derive(Debug, Default)]
pub(crate) struct PageCache {
    pages: HashMap<usize, (u32, RenderedPage)>,
    pending: HashSet<(usize, u32)>,
}

impl PageCache {
    pub(crate) fn claim(&mut self, index: usize, key: u32) -> bool {
        let fresh = self
            .pages
            .get(&index)
            .is_some_and(|(cached, _)| *cached == key);
        !fresh && self.pending.insert((index, key))
    }

    pub(crate) fn image(&self, index: usize) -> Option<&Arc<RenderImage>> {
        self.pages.get(&index).map(|(_, page)| &page.image)
    }

    pub(crate) fn text(&self, index: usize) -> Option<&Arc<PageText>> {
        self.pages.get(&index).map(|(_, page)| &page.text)
    }

    pub(crate) fn store(
        &mut self,
        index: usize,
        key: u32,
        page: Option<RenderedPage>,
        anchor: usize,
        cx: &mut App,
    ) {
        self.pending.remove(&(index, key));
        let Some(page) = page else {
            return;
        };
        if let Some((_, old)) = self.pages.insert(index, (key, page)) {
            cx.drop_image(old.image, None);
        }
        while self.pages.len() > CACHED_PAGES {
            let farthest = self
                .pages
                .keys()
                .copied()
                .max_by_key(|index| index.abs_diff(anchor))
                .expect("cache is not empty");
            if let Some((_, page)) = self.pages.remove(&farthest) {
                cx.drop_image(page.image, None);
            }
        }
    }

    pub(crate) fn clear(&mut self, cx: &mut App) {
        self.pending.clear();
        for (_, (_, page)) in self.pages.drain() {
            cx.drop_image(page.image, None);
        }
    }
}

pub(crate) fn render_task(
    document: &Arc<PdfDocument>,
    index: usize,
    pixel_scale: f32,
    cx: &App,
) -> Task<Option<RenderedPage>> {
    let document = Arc::clone(document);
    run_blocking(cx, move || {
        let page = document.render(index, pixel_scale)?;
        let mut bgra = page.rgba;
        for pixel in bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let buffer = image::RgbaImage::from_raw(page.width, page.height, bgra)?;
        Some(RenderedPage {
            image: Arc::new(RenderImage::new([image::Frame::new(buffer)])),
            text: Arc::new(document.page_text(index)),
        })
    })
}

pub(crate) struct PageLook<'a> {
    pub size: (f32, f32),
    pub scale: f32,
    pub image: Option<&'a Arc<RenderImage>>,
    pub hit: &'a [PageBox],
    pub selection: Vec<PageBox>,
    pub selection_fill: Hsla,
}

fn overlay(bounds: &PageBox, scale: f32) -> gpui_kit::Div {
    div()
        .absolute()
        .left(px(bounds.x * scale))
        .top(px(bounds.y * scale))
        .w(px(bounds.width * scale))
        .h(px(bounds.height * scale))
}

pub(crate) fn page_element(index: usize, look: &PageLook<'_>, palette: &Palette) -> AnyElement {
    let scale = look.scale;
    let page = div()
        .relative()
        .w(px(look.size.0 * scale))
        .h(px(look.size.1 * scale))
        .rounded_xs()
        .shadow_md()
        .children(look.image.map(|image| {
            img(ImageSource::Render(Arc::clone(image)))
                .absolute()
                .size_full()
        }))
        .children(
            look.selection
                .iter()
                .map(|bounds| overlay(bounds, scale).bg(look.selection_fill)),
        )
        .children(look.hit.iter().map(|bounds| {
            overlay(bounds, scale)
                .rounded_xs()
                .border_1()
                .border_color(palette.brand(24.))
                .bg(palette.highlights[0])
        }));
    div()
        .w_full()
        .flex()
        .justify_center()
        .pt(px(page_top(index)))
        .child(page)
        .into_any_element()
}
