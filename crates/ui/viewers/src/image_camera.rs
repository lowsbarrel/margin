use gpui_kit::{Bounds, Pixels, Point};

use crate::controls::{LINE_STEP, PAGE_FRACTION, ScrollKey, ZoomKey};
use crate::image_stage::StageHandle;
use crate::zoom::{IMAGE_ZOOM, ZOOM_STEP, clamp_pan, fit_image, zoom_about};

const PAN_THRESHOLD: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Scale {
    Fit,
    Manual(f32),
}

#[derive(Debug, Clone, Copy)]
struct Pan {
    start: Point<Pixels>,
    origin: (f32, f32),
    moved: bool,
}

#[derive(Debug)]
pub(crate) struct ImageCamera {
    natural: Option<(f32, f32)>,
    scale: Scale,
    stage: StageHandle,
    pan: Option<Pan>,
}

impl ImageCamera {
    pub(crate) fn new() -> Self {
        Self {
            natural: None,
            scale: Scale::Fit,
            stage: StageHandle::default(),
            pan: None,
        }
    }

    pub(crate) const fn stage(&self) -> &StageHandle {
        &self.stage
    }

    pub(crate) const fn natural(&self) -> Option<(f32, f32)> {
        self.natural
    }

    pub(crate) fn set_natural(&mut self, natural: Option<(f32, f32)>) {
        self.natural = natural;
        self.pan = None;
        self.sync();
    }

    pub(crate) fn set_viewport(&self, viewport: Bounds<Pixels>) -> bool {
        if self.stage.get().viewport == viewport {
            return false;
        }
        self.stage.update(|stage| stage.viewport = viewport);
        self.sync();
        true
    }

    pub(crate) fn current_scale(&self) -> f32 {
        match self.scale {
            Scale::Fit => self.natural.map_or(1.0, |natural| {
                fit_image(natural, self.stage.get().viewport_size())
            }),
            Scale::Manual(scale) => scale,
        }
    }

    pub(crate) fn is_fit(&self) -> bool {
        self.scale == Scale::Fit
    }

    pub(crate) const fn is_pressed(&self) -> bool {
        self.pan.is_some()
    }

    pub(crate) fn is_panning(&self) -> bool {
        self.pan.is_some_and(|pan| pan.moved)
    }

    pub(crate) fn content_size(&self) -> (f32, f32) {
        self.content_at(self.current_scale())
    }

    fn content_at(&self, scale: f32) -> (f32, f32) {
        self.natural.map_or((0.0, 0.0), |(width, height)| {
            (
                (width * scale).round().max(1.0),
                (height * scale).round().max(1.0),
            )
        })
    }

    fn sync(&self) {
        let content = self.content_size();
        self.stage.update(|stage| {
            stage.content = content;
            stage.offset = clamp_pan(stage.offset, content, stage.viewport_size());
        });
    }

    pub(crate) fn zoom_by(&mut self, factor: f32, anchor: (f32, f32)) {
        let from = self.current_scale();
        let to = IMAGE_ZOOM.clamp(from * factor);
        self.scale = Scale::Manual(to);
        let content = self.content_at(to);
        self.stage.update(|stage| {
            let offset = zoom_about(stage.offset, anchor, from, to);
            stage.content = content;
            stage.offset = clamp_pan(offset, content, stage.viewport_size());
        });
    }

    pub(crate) fn apply(&mut self, key: ZoomKey) {
        match key {
            ZoomKey::In => self.zoom_by(ZOOM_STEP, (0.0, 0.0)),
            ZoomKey::Out => self.zoom_by(1.0 / ZOOM_STEP, (0.0, 0.0)),
            ZoomKey::Actual => {
                let scroll = self.stage.get().scroll();
                self.scale = Scale::Manual(1.0);
                let content = self.content_at(1.0);
                self.stage.update(|stage| {
                    stage.content = content;
                    stage.set_scroll(scroll);
                });
            }
            ZoomKey::Fit => {
                self.scale = Scale::Fit;
                self.sync();
            }
        }
    }

    pub(crate) fn scroll_by(&self, dx: f32, dy: f32) {
        self.stage.update(|stage| {
            let offset = (stage.offset.0 + dx, stage.offset.1 + dy);
            stage.offset = clamp_pan(offset, stage.content, stage.viewport_size());
        });
    }

    pub(crate) fn scroll_key(&self, key: ScrollKey) {
        match key {
            ScrollKey::Line(x, y) => self.scroll_by(-x * LINE_STEP, -y * LINE_STEP),
            ScrollKey::Page(direction) => {
                let height = self.stage.get().viewport_size().1;
                self.scroll_by(0.0, -direction * height * PAGE_FRACTION);
            }
            ScrollKey::Start => self.stage.update(|stage| {
                let scroll = stage.scroll();
                stage.set_scroll((scroll.0, 0.0));
            }),
            ScrollKey::End => self.stage.update(|stage| {
                let scroll = stage.scroll();
                stage.set_scroll((scroll.0, f32::MAX));
            }),
        }
    }

    pub(crate) fn start_pan(&mut self, position: Point<Pixels>) {
        if self.natural.is_some() {
            self.pan = Some(Pan {
                start: position,
                origin: self.stage.get().offset,
                moved: false,
            });
        }
    }

    pub(crate) fn move_pan(&mut self, position: Point<Pixels>) -> bool {
        let Some(pan) = self.pan.as_mut() else {
            return false;
        };
        let dx = f32::from(position.x - pan.start.x);
        let dy = f32::from(position.y - pan.start.y);
        if !pan.moved {
            if dx.abs() + dy.abs() <= PAN_THRESHOLD {
                return false;
            }
            pan.moved = true;
        }
        let origin = pan.origin;
        self.stage.update(|stage| {
            let offset = (origin.0 + dx, origin.1 + dy);
            stage.offset = clamp_pan(offset, stage.content, stage.viewport_size());
        });
        true
    }

    pub(crate) fn end_pan(&mut self) -> bool {
        self.pan.take().is_some_and(|pan| pan.moved)
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{point, px, size};

    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn camera(natural: (f32, f32)) -> ImageCamera {
        let mut camera = ImageCamera::new();
        camera.set_viewport(Bounds::new(point(px(0.), px(0.)), size(px(800.), px(500.))));
        camera.set_natural(Some(natural));
        camera
    }

    #[test]
    fn opens_fitted_and_zoom_keys_step_from_the_fitted_scale() {
        let mut camera = camera((1600.0, 1000.0));
        assert!(camera.is_fit() && close(camera.current_scale(), 0.5));
        camera.apply(ZoomKey::In);
        assert!(!camera.is_fit() && close(camera.current_scale(), 0.625));
        camera.apply(ZoomKey::Out);
        camera.apply(ZoomKey::Out);
        assert!(close(camera.current_scale(), 0.4));
        camera.apply(ZoomKey::Fit);
        assert!(camera.is_fit() && close(camera.current_scale(), 0.5));
        assert_eq!(camera.stage().get().offset, (0.0, 0.0));
    }

    #[test]
    fn actual_size_from_fit_shows_the_top_left_corner() {
        let mut camera = camera((1600.0, 1000.0));
        camera.apply(ZoomKey::Actual);
        assert!(close(camera.current_scale(), 1.0));
        let stage = camera.stage().get();
        assert_eq!(stage.content, (1600.0, 1000.0));
        assert_eq!(stage.scroll(), (0.0, 0.0));
    }

    #[test]
    fn zoom_is_clamped_and_small_images_are_never_upscaled_to_fit() {
        let mut camera = camera((200.0, 100.0));
        assert!(close(camera.current_scale(), 1.0));
        for _ in 0..40 {
            camera.apply(ZoomKey::In);
        }
        assert!(close(camera.current_scale(), IMAGE_ZOOM.max));
        for _ in 0..80 {
            camera.apply(ZoomKey::Out);
        }
        assert!(close(camera.current_scale(), IMAGE_ZOOM.min));
    }

    #[test]
    fn panning_waits_for_the_threshold_and_stays_in_bounds() {
        let mut camera = camera((1600.0, 1000.0));
        camera.apply(ZoomKey::Actual);
        camera.start_pan(point(px(400.), px(250.)));
        assert!(!camera.move_pan(point(px(402.), px(251.))));
        assert!(!camera.is_panning());
        assert!(camera.move_pan(point(px(300.), px(200.))));
        assert!(camera.is_panning());
        assert_eq!(camera.stage().get().scroll(), (100.0, 50.0));
        assert!(camera.move_pan(point(px(-5000.), px(-5000.))));
        assert_eq!(camera.stage().get().scroll(), (800.0, 500.0));
        assert!(camera.end_pan());
        assert!(!camera.is_panning());
    }

    #[test]
    fn keys_scroll_like_a_focused_scroll_area() {
        let mut camera = camera((1600.0, 1000.0));
        camera.apply(ZoomKey::Actual);
        camera.scroll_key(ScrollKey::Line(0.0, 1.0));
        assert_eq!(camera.stage().get().scroll(), (0.0, LINE_STEP));
        camera.scroll_key(ScrollKey::End);
        assert_eq!(camera.stage().get().scroll(), (0.0, 500.0));
        camera.scroll_key(ScrollKey::Start);
        assert_eq!(camera.stage().get().scroll(), (0.0, 0.0));
    }
}
