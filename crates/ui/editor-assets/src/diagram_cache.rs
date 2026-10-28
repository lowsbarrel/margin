use std::mem;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{App, Global, Hsla, Image, ImageFormat, SharedString, Window};
use margin_diagrams::{DiagramStyle, Error, MathMode, Svg};
use margin_theme::MarginTheme;
use margin_workbench::run_blocking;

use crate::generations::Generations;

const CACHED_DIAGRAMS: usize = 128;
const NOTE_BODY_TO_UI_SIZE: f32 = 15.0 / 14.0;
const KATEX_FONT_SCALE: f32 = 1.21;

#[derive(Clone, Debug, PartialEq)]
pub struct DiagramImage {
    pub image: Arc<Image>,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagramState {
    Empty,
    Pending,
    Ready(DiagramImage),
    Failed(SharedString),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum DiagramSource {
    Math { tex: String, mode: MathKey },
    Mermaid(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum MathKey {
    Inline,
    Display,
}

impl From<MathMode> for MathKey {
    fn from(mode: MathMode) -> Self {
        match mode {
            MathMode::Inline => Self::Inline,
            MathMode::Display => Self::Display,
        }
    }
}

#[derive(Debug)]
struct Entry {
    state: DiagramState,
    rendered_with: Option<DiagramStyle>,
    requested_with: DiagramStyle,
}

#[derive(Debug)]
pub struct DiagramCache {
    entries: Generations<DiagramSource, Entry>,
    retired: Vec<Arc<Image>>,
}

impl Default for DiagramCache {
    fn default() -> Self {
        Self {
            entries: Generations::with_capacity(CACHED_DIAGRAMS),
            retired: Vec::new(),
        }
    }
}

impl Global for DiagramCache {}

impl DiagramCache {
    pub fn release_unused(window: &mut Window, cx: &mut App) {
        let Some(cache) = cx.try_global::<Self>() else {
            return;
        };
        if cache.retired.is_empty() && !cache.entries.has_evicted() {
            return;
        }
        let cache = cx.global_mut::<Self>();
        let mut images = mem::take(&mut cache.retired);
        images.extend(cache.entries.take_evicted().into_iter().filter_map(
            |entry| match entry.state {
                DiagramState::Ready(diagram) => Some(diagram.image),
                _ => None,
            },
        ));
        for image in images {
            if let Some(render) = Arc::clone(&image).get_render_image(window, cx) {
                cx.drop_image(render, Some(window));
            }
            image.remove_asset(cx);
        }
    }
}

#[must_use]
pub fn math(tex: &str, mode: MathMode, cx: &mut App) -> DiagramState {
    if tex.trim().is_empty() {
        return DiagramState::Empty;
    }
    request(
        DiagramSource::Math {
            tex: tex.to_owned(),
            mode: mode.into(),
        },
        cx,
    )
}

#[must_use]
pub fn mermaid(source: &str, cx: &mut App) -> DiagramState {
    if source.trim().is_empty() {
        return DiagramState::Empty;
    }
    request(DiagramSource::Mermaid(source.to_owned()), cx)
}

fn request(source: DiagramSource, cx: &mut App) -> DiagramState {
    let style = diagram_style(cx);
    if let Some(entry) = cx
        .try_global::<DiagramCache>()
        .and_then(|cache| cache.entries.get_current(&source))
        && (entry.rendered_with.as_ref() == Some(&style) || entry.requested_with == style)
    {
        return entry.state.clone();
    }
    let cache = cx.default_global::<DiagramCache>();
    if let Some(entry) = cache.entries.get_mut(&source) {
        if entry.rendered_with.as_ref() == Some(&style) || entry.requested_with == style {
            return entry.state.clone();
        }
        entry.requested_with = style.clone();
        let state = entry.state.clone();
        start_render(source, style, cx);
        return state;
    }
    cache.entries.insert(
        source.clone(),
        Entry {
            state: DiagramState::Pending,
            rendered_with: None,
            requested_with: style.clone(),
        },
    );
    start_render(source, style, cx);
    DiagramState::Pending
}

fn start_render(source: DiagramSource, style: DiagramStyle, cx: &App) {
    let key = source.clone();
    let used = style.clone();
    let render = run_blocking(cx, move || rendered(&source, &style));
    cx.spawn(async move |cx| {
        let state = render.await;
        cx.update_global::<DiagramCache, _>(|cache, _| {
            if let Some(entry) = cache.entries.get_mut(&key)
                && entry.requested_with == used
            {
                if let DiagramState::Ready(old) = mem::replace(&mut entry.state, state) {
                    cache.retired.push(old.image);
                }
                entry.rendered_with = Some(used);
            }
        });
    })
    .detach();
}

fn rendered(source: &DiagramSource, style: &DiagramStyle) -> DiagramState {
    let result = match source {
        DiagramSource::Math { tex, mode } => {
            let mode = match mode {
                MathKey::Inline => MathMode::Inline,
                MathKey::Display => MathMode::Display,
            };
            let math_style = DiagramStyle {
                font_size: style.font_size * KATEX_FONT_SCALE,
                ..style.clone()
            };
            margin_diagrams::render_math(tex, mode, &math_style)
        }
        DiagramSource::Mermaid(code) => margin_diagrams::render_mermaid(code, style),
    };
    state_of(result)
}

fn state_of(result: Result<Svg, Error>) -> DiagramState {
    match result {
        Ok(svg) => DiagramState::Ready(DiagramImage {
            image: Arc::new(Image::from_bytes(ImageFormat::Svg, svg.bytes.to_vec())),
            width: svg.width,
            height: svg.height,
            depth: svg.depth,
        }),
        Err(error) => DiagramState::Failed(error.to_string().into()),
    }
}

fn diagram_style(cx: &App) -> DiagramStyle {
    let palette = MarginTheme::palette(cx);
    DiagramStyle {
        foreground: hex(palette.text_primary),
        background: hex(palette.surface[0]),
        accent: hex(palette.text_brand),
        muted: hex(palette.text_secondary),
        font_size: f32::from(cx.theme().font_size) * NOTE_BODY_TO_UI_SIZE,
        dark: palette.dark,
    }
}

fn hex(color: Hsla) -> String {
    format!("#{:06x}", u32::from(color.to_rgb()) >> 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_are_written_as_rrggbb() {
        assert_eq!(hex(Hsla::white()), "#ffffff");
        assert_eq!(hex(Hsla::red().opacity(0.4)), "#ff0000");
    }

    #[test]
    fn math_sources_differ_by_mode() {
        let inline = DiagramSource::Math {
            tex: String::from("x"),
            mode: MathMode::Inline.into(),
        };
        let display = DiagramSource::Math {
            tex: String::from("x"),
            mode: MathMode::Display.into(),
        };
        assert_ne!(inline, display);
        assert_ne!(inline, DiagramSource::Mermaid(String::from("x")));
    }
}
