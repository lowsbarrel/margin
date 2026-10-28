pub const ZOOM_STEP: f32 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomRange {
    pub min: f32,
    pub max: f32,
}

pub const IMAGE_ZOOM: ZoomRange = ZoomRange { min: 0.1, max: 8.0 };
pub const PDF_ZOOM: ZoomRange = ZoomRange {
    min: 0.25,
    max: 6.0,
};
pub(crate) const PDF_PAGE_PADDING: f32 = 32.0;

impl ZoomRange {
    #[must_use]
    pub fn clamp(self, scale: f32) -> f32 {
        scale.clamp(self.min, self.max)
    }
}

#[must_use]
pub fn fit_image(natural: (f32, f32), viewport: (f32, f32)) -> f32 {
    if natural.0 <= 0.0 || natural.1 <= 0.0 || viewport.0 <= 0.0 || viewport.1 <= 0.0 {
        return 1.0;
    }
    (viewport.0 / natural.0)
        .min(viewport.1 / natural.1)
        .min(1.0)
}

#[must_use]
pub fn fit_width(page_width: f32, container_width: f32) -> f32 {
    if page_width <= 0.0 || container_width <= 0.0 {
        return 1.0;
    }
    PDF_ZOOM.clamp((container_width - PDF_PAGE_PADDING) / page_width)
}

#[must_use]
pub fn wheel_factor(delta_y: f32) -> f32 {
    (-delta_y * 0.005).exp().clamp(0.5, 2.0)
}

#[must_use]
pub fn zoom_about(offset: (f32, f32), anchor: (f32, f32), from: f32, to: f32) -> (f32, f32) {
    let ratio = to / from;
    (
        (anchor.0 - offset.0).mul_add(-ratio, anchor.0),
        (anchor.1 - offset.1).mul_add(-ratio, anchor.1),
    )
}

#[must_use]
pub fn clamp_pan(offset: (f32, f32), content: (f32, f32), viewport: (f32, f32)) -> (f32, f32) {
    let limit_x = ((content.0 - viewport.0) / 2.0).max(0.0);
    let limit_y = ((content.1 - viewport.1) / 2.0).max(0.0);
    (
        offset.0.clamp(-limit_x, limit_x),
        offset.1.clamp(-limit_y, limit_y),
    )
}

#[must_use]
pub(crate) fn scroll_from_offset(offset: f32, content: f32, viewport: f32) -> f32 {
    ((content - viewport) / 2.0 - offset).max(0.0)
}

#[must_use]
pub(crate) fn offset_from_scroll(scroll: f32, content: f32, viewport: f32) -> f32 {
    let limit = ((content - viewport) / 2.0).max(0.0);
    (limit - scroll).clamp(-limit, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn scroll_and_offset_convert_both_ways() {
        assert!(close(scroll_from_offset(0.0, 1000.0, 400.0), 300.0));
        assert!(close(scroll_from_offset(300.0, 1000.0, 400.0), 0.0));
        assert!(close(scroll_from_offset(0.0, 200.0, 400.0), 0.0));
        assert!(close(offset_from_scroll(0.0, 1000.0, 400.0), 300.0));
        assert!(close(offset_from_scroll(600.0, 1000.0, 400.0), -300.0));
        assert!(close(offset_from_scroll(9_000.0, 1000.0, 400.0), -300.0));
        assert!(close(offset_from_scroll(50.0, 200.0, 400.0), 0.0));
        let offset = offset_from_scroll(123.0, 1000.0, 400.0);
        assert!(close(scroll_from_offset(offset, 1000.0, 400.0), 123.0));
    }

    #[test]
    fn fit_never_upscales_and_respects_both_axes() {
        assert!(close(fit_image((100.0, 50.0), (800.0, 600.0)), 1.0));
        assert!(close(fit_image((2000.0, 1000.0), (1000.0, 1000.0)), 0.5));
        assert!(close(fit_image((1000.0, 2000.0), (1000.0, 1000.0)), 0.5));
        assert!(close(fit_image((0.0, 0.0), (1000.0, 1000.0)), 1.0));
    }

    #[test]
    fn fit_width_leaves_padding_and_clamps() {
        assert!(close(fit_width(612.0, 644.0), 1.0));
        assert!(close(fit_width(612.0, 100_000.0), PDF_ZOOM.max));
        assert!(close(fit_width(612.0, 40.0), PDF_ZOOM.min));
    }

    #[test]
    fn zoom_about_keeps_the_anchor_point_fixed() {
        let (from, to) = (1.0, 2.0);
        let offset = zoom_about((10.0, -5.0), (100.0, 50.0), from, to);
        let before = ((100.0 - 10.0) / from, (50.0 + 5.0) / from);
        let after = ((100.0 - offset.0) / to, (50.0 - offset.1) / to);
        assert!(close(before.0, after.0) && close(before.1, after.1));
    }

    #[test]
    fn wheel_and_pan_are_bounded() {
        assert!(close(wheel_factor(-10_000.0), 2.0));
        assert!(close(wheel_factor(10_000.0), 0.5));
        assert!(wheel_factor(-10.0) > 1.0);
        assert_eq!(
            clamp_pan((500.0, -500.0), (300.0, 900.0), (200.0, 500.0)),
            (50.0, -200.0)
        );
        assert_eq!(
            clamp_pan((5.0, 5.0), (100.0, 100.0), (200.0, 200.0)),
            (0.0, 0.0)
        );
    }
}
