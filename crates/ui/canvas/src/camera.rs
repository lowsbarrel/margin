#![expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "crate-private state shared between the canvas modules"
)]

use crate::model::WorldPoint;

const MIN_ZOOM: f64 = 0.1;
const MAX_ZOOM: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Camera {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Camera {
    pub(crate) fn to_world(self, local_x: f64, local_y: f64) -> WorldPoint {
        WorldPoint::new(local_x / self.zoom + self.x, local_y / self.zoom + self.y)
    }

    pub(crate) fn to_local(self, world: WorldPoint) -> (f64, f64) {
        (
            (world.x - self.x) * self.zoom,
            (world.y - self.y) * self.zoom,
        )
    }

    pub(crate) fn pan_by(&mut self, delta_x: f64, delta_y: f64) {
        self.x += delta_x / self.zoom;
        self.y += delta_y / self.zoom;
    }

    pub(crate) fn zoom_at(&mut self, local_x: f64, local_y: f64, factor: f64) {
        let next = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let anchor = self.to_world(local_x, local_y);
        self.x = anchor.x - local_x / next;
        self.y = anchor.y - local_y / next;
        self.zoom = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_and_local_round_trip() {
        let camera = Camera {
            x: 120.0,
            y: -40.0,
            zoom: 2.0,
        };
        let world = camera.to_world(50.0, 30.0);
        assert_eq!(world, WorldPoint::new(145.0, -25.0));
        assert_eq!(camera.to_local(world), (50.0, 30.0));
    }

    #[test]
    fn zoom_keeps_the_anchor_under_the_cursor_and_clamps() {
        let mut camera = Camera::default();
        let before = camera.to_world(200.0, 100.0);
        camera.zoom_at(200.0, 100.0, 1.1);
        let after = camera.to_world(200.0, 100.0);
        assert!(before.distance(after) < 1e-9);
        camera.zoom_at(0.0, 0.0, 1000.0);
        assert!((camera.zoom - MAX_ZOOM).abs() < f64::EPSILON);
        camera.zoom_at(0.0, 0.0, 0.0001);
        assert!((camera.zoom - MIN_ZOOM).abs() < f64::EPSILON);
    }

    #[test]
    fn pan_moves_in_world_units() {
        let mut camera = Camera {
            zoom: 2.0,
            ..Camera::default()
        };
        camera.pan_by(10.0, -20.0);
        assert_eq!((camera.x, camera.y), (5.0, -10.0));
    }
}
