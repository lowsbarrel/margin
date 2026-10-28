#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spot {
    pub x: f32,
    pub y: f32,
}

pub(crate) const fn spot(x: f32, y: f32) -> Spot {
    Spot { x, y }
}

impl Spot {
    pub(crate) const fn translated(self, by: Self) -> Self {
        spot(self.x + by.x, self.y + by.y)
    }

    pub(crate) fn rotated_around(self, pivot: Self, degrees: f32) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let dx = self.x - pivot.x;
        let dy = self.y - pivot.y;
        spot(
            dx.mul_add(cos, -dy * sin) + pivot.x,
            dx.mul_add(sin, dy * cos) + pivot.y,
        )
    }

    pub(crate) fn distance(self, other: Self) -> f32 {
        (other.x - self.x).hypot(other.y - self.y)
    }

    pub(crate) fn lerp(self, other: Self, amount: f32) -> Self {
        spot(
            (other.x - self.x).mul_add(amount, self.x),
            (other.y - self.y).mul_add(amount, self.y),
        )
    }

    pub(crate) fn bezier(self, control_a: Self, control_b: Self, to: Self, amount: f32) -> Self {
        let rest = 1.0 - amount;
        let weights = [
            rest * rest * rest,
            3.0 * rest * rest * amount,
            3.0 * rest * amount * amount,
            amount * amount * amount,
        ];
        let points = [self, control_a, control_b, to];
        let mix = |coordinate: fn(Self) -> f32| {
            weights
                .iter()
                .zip(points)
                .map(|(weight, point)| weight * coordinate(point))
                .sum()
        };
        spot(mix(|point| point.x), mix(|point| point.y))
    }
}

pub(crate) fn arc_points(
    center: Spot,
    radius: f32,
    from_degrees: f32,
    to_degrees: f32,
) -> Vec<Spot> {
    const SEGMENTS: u8 = 24;
    (0..=SEGMENTS)
        .map(|index| {
            let degrees = (to_degrees - from_degrees)
                .mul_add(f32::from(index) / f32::from(SEGMENTS), from_degrees);
            let (sin, cos) = degrees.to_radians().sin_cos();
            spot(radius.mul_add(cos, center.x), radius.mul_add(sin, center.y))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_a_spot_around_its_pivot() {
        let turned = spot(10.0, 0.0).rotated_around(spot(0.0, 0.0), 90.0);
        assert!(turned.x.abs() < 1e-4 && (turned.y - 10.0).abs() < 1e-4);
    }

    #[test]
    fn arc_points_run_from_start_to_end_angle() {
        let points = arc_points(spot(0.0, 0.0), 2.0, 180.0, 360.0);
        let (first, last) = (points[0], points[points.len() - 1]);
        assert!((first.x + 2.0).abs() < 1e-4 && first.y.abs() < 1e-4);
        assert!((last.x - 2.0).abs() < 1e-4 && last.y.abs() < 1e-4);
    }
}
