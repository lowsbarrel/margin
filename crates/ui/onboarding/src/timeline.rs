use std::time::Duration;

use gpui_kit::Animation;
use margin_theme::MOTION_MEDIUM;

const LENGTH: f32 = 1700.0;
const OVERSHOOT: f32 = 1.701_58;

pub(crate) fn entrance() -> Animation {
    Animation::new(MOTION_MEDIUM * 4)
}

pub(crate) fn transition(duration: Duration) -> Animation {
    Animation::new(duration).with_easing(ease_out)
}

pub(crate) fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Timeline(f32);

impl Timeline {
    pub(crate) const fn at(progress: f32) -> Self {
        Self(progress * LENGTH)
    }

    fn local(self, start: f32, span: f32) -> f32 {
        ((self.0 - start) / span).clamp(0.0, 1.0)
    }

    pub(crate) fn ease(self, start: f32, span: f32) -> f32 {
        ease_out(self.local(start, span))
    }

    pub(crate) fn spring(self, start: f32, span: f32) -> f32 {
        let shifted = self.local(start, span) - 1.0;
        (OVERSHOOT + 1.0).mul_add(shifted.powi(3), OVERSHOOT * shifted.powi(2)) + 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyframes_ease_from_zero_to_one_inside_their_window() {
        let start = Timeline::at(0.0);
        let end = Timeline::at(1.0);
        assert!(start.ease(100.0, 200.0).abs() < f32::EPSILON);
        assert!((end.ease(100.0, 200.0) - 1.0).abs() < f32::EPSILON);
        let middle = Timeline::at(200.0 / LENGTH).ease(100.0, 200.0);
        assert!(middle > 0.5 && middle < 1.0);
    }

    #[test]
    fn the_spring_overshoots_then_settles() {
        let peak = Timeline::at(1200.0 / LENGTH).spring(1000.0, 300.0);
        assert!(peak > 1.0);
        assert!((Timeline::at(1.0).spring(1000.0, 300.0) - 1.0).abs() < 1e-5);
        assert!(Timeline::at(0.0).spring(1000.0, 300.0).abs() < 1e-5);
    }
}
