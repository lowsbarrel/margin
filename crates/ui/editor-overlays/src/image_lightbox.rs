use std::fmt;

use gpui_kit::{
    AnyElement, App, Bounds, Context, CursorStyle, EventEmitter, FocusHandle, Focusable,
    ImageSource, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ObjectFit, ParentElement, PinchEvent, Pixels, Point, Render,
    Role, ScrollEvent, SharedString, StatefulInteractiveElement, Styled, StyledImage, Window,
    canvas, div, img, px, size,
};
use margin_i18n::t;
use margin_theme::MarginTheme;
use margin_viewers::{
    IMAGE_ZOOM, ZOOM_STEP, ZoomKey, clamp_pan, fit_image, wheel_factor, zoom_about, zoom_key,
};

use crate::lightbox_chrome::{counter, lightbox_button, lightbox_close, zoom_bar};
use crate::lightbox_source::{Loaded, load};

const DRAG_THRESHOLD: f32 = 3.0;

#[derive(Clone)]
pub struct LightboxImage {
    pub source: ImageSource,
    pub alt: SharedString,
}

impl fmt::Debug for LightboxImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LightboxImage")
            .field("alt", &self.alt)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightboxEvent {
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scale {
    Fit,
    Manual(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Step {
    Previous,
    Next,
}

#[derive(Debug)]
struct Drag {
    start: Point<Pixels>,
    origin: (f32, f32),
    moved: bool,
}

#[derive(Debug)]
pub struct ImageLightbox {
    focus_handle: FocusHandle,
    images: Vec<LightboxImage>,
    index: usize,
    scale: Scale,
    offset: (f32, f32),
    viewport: Bounds<Pixels>,
    natural: Option<(f32, f32)>,
    drag: Option<Drag>,
}

impl EventEmitter<LightboxEvent> for ImageLightbox {}

impl Focusable for ImageLightbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ImageLightbox {
    #[must_use]
    pub fn new(
        images: Vec<LightboxImage>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            focus_handle,
            index: index.min(images.len().saturating_sub(1)),
            images,
            scale: Scale::Fit,
            offset: (0.0, 0.0),
            viewport: Bounds::default(),
            natural: None,
            drag: None,
        }
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn is_fit(&self) -> bool {
        self.scale == Scale::Fit
    }

    pub(crate) fn close(cx: &mut Context<'_, Self>) {
        cx.emit(LightboxEvent::Close);
    }

    pub(crate) fn step(&mut self, step: Step, cx: &mut Context<'_, Self>) {
        let total = self.images.len();
        if total < 2 {
            return;
        }
        self.index = match step {
            Step::Previous => (self.index + total - 1) % total,
            Step::Next => (self.index + 1) % total,
        };
        self.scale = Scale::Fit;
        self.offset = (0.0, 0.0);
        self.natural = None;
        cx.notify();
    }

    fn viewport_size(&self) -> (f32, f32) {
        (
            f32::from(self.viewport.size.width),
            f32::from(self.viewport.size.height),
        )
    }

    pub(crate) fn current_scale(&self) -> f32 {
        match self.scale {
            Scale::Fit => self
                .natural
                .map_or(1.0, |natural| fit_image(natural, self.viewport_size())),
            Scale::Manual(scale) => scale,
        }
    }

    fn content_size(&self, scale: f32) -> (f32, f32) {
        self.natural.map_or((0.0, 0.0), |(width, height)| {
            (width * scale, height * scale)
        })
    }

    fn zoom_to(&mut self, next: f32, anchor: (f32, f32), cx: &mut Context<'_, Self>) {
        if self.natural.is_none() {
            return;
        }
        let from = self.current_scale();
        let to = IMAGE_ZOOM.clamp(next);
        let offset = zoom_about(self.offset, anchor, from, to);
        self.scale = Scale::Manual(to);
        self.offset = clamp_pan(offset, self.content_size(to), self.viewport_size());
        cx.notify();
    }

    pub(crate) fn apply_zoom(&mut self, key: ZoomKey, cx: &mut Context<'_, Self>) {
        let scale = self.current_scale();
        match key {
            ZoomKey::In => self.zoom_to(scale * ZOOM_STEP, (0.0, 0.0), cx),
            ZoomKey::Out => self.zoom_to(scale / ZOOM_STEP, (0.0, 0.0), cx),
            ZoomKey::Actual => self.zoom_to(1.0, (0.0, 0.0), cx),
            ZoomKey::Fit => {
                self.scale = Scale::Fit;
                self.offset = (0.0, 0.0);
                cx.notify();
            }
        }
    }

    fn anchor(&self, position: Point<Pixels>) -> (f32, f32) {
        let center = self.viewport.center();
        (
            f32::from(position.x - center.x),
            f32::from(position.y - center.y),
        )
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        match event.keystroke.key.as_str() {
            "escape" => Self::close(cx),
            "left" => self.step(Step::Previous, cx),
            "right" => self.step(Step::Next, cx),
            _ => match zoom_key(event, cx) {
                Some(key) => self.apply_zoom(key, cx),
                None => return,
            },
        }
        cx.stop_propagation();
    }

    fn on_wheel(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let delta = event.delta.pixel_delta(px(16.0));
        if event.modifiers.secondary(cx) {
            let anchor = self.anchor(event.position);
            self.zoom_to(
                self.current_scale() * wheel_factor(-f32::from(delta.y)),
                anchor,
                cx,
            );
        } else {
            self.pan_to(
                (
                    self.offset.0 + f32::from(delta.x),
                    self.offset.1 + f32::from(delta.y),
                ),
                cx,
            );
        }
    }

    fn on_pinch(&mut self, event: &PinchEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let anchor = self.anchor(event.position);
        self.zoom_to(self.current_scale() * (1.0 + event.delta), anchor, cx);
    }

    fn pan_to(&mut self, offset: (f32, f32), cx: &mut Context<'_, Self>) {
        let content = self.content_size(self.current_scale());
        self.offset = clamp_pan(offset, content, self.viewport_size());
        cx.notify();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.focus_handle.focus(window, cx);
        self.drag = Some(Drag {
            start: event.position,
            origin: self.offset,
            moved: false,
        });
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let dx = f32::from(event.position.x - drag.start.x);
        let dy = f32::from(event.position.y - drag.start.y);
        if !drag.moved && dx.abs() + dy.abs() <= DRAG_THRESHOLD {
            return;
        }
        drag.moved = true;
        let origin = drag.origin;
        self.pan_to((origin.0 + dx, origin.1 + dy), cx);
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if !drag.moved && !self.image_bounds().contains(&event.position) {
            Self::close(cx);
        }
    }

    fn image_bounds(&self) -> Bounds<Pixels> {
        let (width, height) = self.content_size(self.current_scale());
        let center = self.viewport.center();
        Bounds::centered_at(
            Point::new(center.x + px(self.offset.0), center.y + px(self.offset.1)),
            size(px(width), px(height)),
        )
    }

    fn render_image(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> AnyElement {
        let failed_color = MarginTheme::palette(cx).lightbox_control;
        let Some(image) = self.images.get(self.index) else {
            return div().into_any_element();
        };
        let source = image.source.clone();
        match load(&source, window, cx) {
            Loaded::Pending => div().into_any_element(),
            Loaded::Failed => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(failed_color)
                .child(t(cx, "viewer_image_failed"))
                .into_any_element(),
            Loaded::Unsized => img(source)
                .size_full()
                .object_fit(ObjectFit::ScaleDown)
                .into_any_element(),
            Loaded::Ready(render) => {
                let pixels = render.size(0);
                #[expect(clippy::cast_precision_loss, reason = "image sizes fit in f32")]
                let natural = (pixels.width.0 as f32, pixels.height.0 as f32);
                self.natural = Some(natural);
                let bounds = self.image_bounds();
                let origin = self.viewport.origin;
                img(ImageSource::Render(render))
                    .absolute()
                    .left(bounds.origin.x - origin.x)
                    .top(bounds.origin.y - origin.y)
                    .w(bounds.size.width.max(px(1.0)))
                    .h(bounds.size.height.max(px(1.0)))
                    .shadow_lg()
                    .into_any_element()
            }
        }
    }
}

impl Render for ImageLightbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let backdrop = MarginTheme::palette(cx).lightbox_backdrop;
        let entity = cx.entity();
        let tracker = canvas(
            move |bounds, _, cx| {
                entity.update(cx, |lightbox, cx| {
                    if lightbox.viewport != bounds {
                        lightbox.viewport = bounds;
                        cx.notify();
                    }
                });
            },
            |_, (), _, _| {},
        )
        .absolute()
        .size_full();
        let name = self
            .images
            .get(self.index)
            .map(|image| image.alt.clone())
            .unwrap_or_default();
        let total = self.images.len();
        let image = self.render_image(window, cx);
        let mut root = div()
            .id("image-lightbox")
            .role(Role::Dialog)
            .aria_label(name)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .overflow_hidden()
            .bg(backdrop)
            .cursor(if self.drag.as_ref().is_some_and(|drag| drag.moved) {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .on_scroll(cx.listener(Self::on_wheel))
            .on_pinch(cx.listener(Self::on_pinch))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .child(tracker)
            .child(image)
            .child(zoom_bar(self, cx));
        if total > 1 {
            root = root
                .child(counter(self.index + 1, total, cx))
                .child(lightbox_button("lightbox-previous", Step::Previous, cx))
                .child(lightbox_button("lightbox-next", Step::Next, cx));
        }
        root.child(lightbox_close(cx))
    }
}
