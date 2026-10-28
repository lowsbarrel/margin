use crate::model::{Shape, ShapeKind, TextLabel, WorldPoint};

const SNAP_THRESHOLD: f64 = 12.0;

pub(crate) fn snap_points(
    shapes: &[Shape],
    labels: &[TextLabel],
    widths: &[f64],
) -> Vec<WorldPoint> {
    let mut points = Vec::new();
    for shape in shapes {
        match shape.kind {
            ShapeKind::Rect => {
                let (left, right) = (shape.x1.min(shape.x2), shape.x1.max(shape.x2));
                let (top, bottom) = (shape.y1.min(shape.y2), shape.y1.max(shape.y2));
                let (middle_x, middle_y) = (f64::midpoint(left, right), f64::midpoint(top, bottom));
                points.extend([
                    WorldPoint::new(left, top),
                    WorldPoint::new(right, top),
                    WorldPoint::new(right, bottom),
                    WorldPoint::new(left, bottom),
                    WorldPoint::new(middle_x, top),
                    WorldPoint::new(right, middle_y),
                    WorldPoint::new(middle_x, bottom),
                    WorldPoint::new(left, middle_y),
                ]);
            }
            ShapeKind::Ellipse => {
                let (center_x, center_y) = (
                    f64::midpoint(shape.x1, shape.x2),
                    f64::midpoint(shape.y1, shape.y2),
                );
                let radius_x = (shape.x2 - shape.x1).abs() / 2.0;
                let radius_y = (shape.y2 - shape.y1).abs() / 2.0;
                points.extend([
                    WorldPoint::new(center_x, center_y - radius_y),
                    WorldPoint::new(center_x + radius_x, center_y),
                    WorldPoint::new(center_x, center_y + radius_y),
                    WorldPoint::new(center_x - radius_x, center_y),
                ]);
            }
            ShapeKind::Line | ShapeKind::Arrow => points.extend([shape.start(), shape.end()]),
        }
    }
    for (index, label) in labels.iter().enumerate() {
        let width = widths
            .get(index)
            .copied()
            .unwrap_or_else(|| label.estimated_width());
        let height = label.line_height();
        points.extend([
            WorldPoint::new(label.x, label.y),
            WorldPoint::new(label.x + width / 2.0, label.y),
            WorldPoint::new(label.x + width, label.y),
            WorldPoint::new(label.x + width, label.y + height),
            WorldPoint::new(label.x, label.y + height),
            WorldPoint::new(label.x + width / 2.0, label.y + height / 2.0),
        ]);
    }
    points
}

pub(crate) fn find_nearest(
    points: &[WorldPoint],
    target: WorldPoint,
    zoom: f64,
) -> Option<WorldPoint> {
    let mut best = None;
    let mut best_distance = SNAP_THRESHOLD / zoom;
    for point in points {
        let distance = point.distance(target);
        if distance < best_distance {
            best_distance = distance;
            best = Some(*point);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::INK_COLOR;

    fn shape(kind: ShapeKind, x1: f64, y1: f64, x2: f64, y2: f64) -> Shape {
        Shape {
            kind,
            x1,
            y1,
            x2,
            y2,
            color: INK_COLOR.to_owned(),
            size: 3.0,
        }
    }

    #[test]
    fn snaps_to_rect_corners_midpoints_and_label_anchors() {
        let shapes = [shape(ShapeKind::Rect, 100.0, 100.0, 0.0, 0.0)];
        let labels = [TextLabel {
            x: 200.0,
            y: 0.0,
            text: "hi".to_owned(),
            color: INK_COLOR.to_owned(),
            font_size: 10.0,
        }];
        let points = snap_points(&shapes, &labels, &[40.0]);
        assert_eq!(points.len(), 14);
        assert_eq!(
            find_nearest(&points, WorldPoint::new(52.0, -3.0), 1.0),
            Some(WorldPoint::new(50.0, 0.0))
        );
        assert_eq!(
            find_nearest(&points, WorldPoint::new(238.0, 11.0), 1.0),
            Some(WorldPoint::new(240.0, 12.0))
        );
    }

    #[test]
    fn threshold_shrinks_in_world_units_as_zoom_grows() {
        let points = snap_points(&[shape(ShapeKind::Line, 0.0, 0.0, 50.0, 0.0)], &[], &[]);
        let near = WorldPoint::new(8.0, 0.0);
        assert_eq!(
            find_nearest(&points, near, 1.0),
            Some(WorldPoint::new(0.0, 0.0))
        );
        assert_eq!(find_nearest(&points, near, 2.0), None);
    }

    #[test]
    fn ellipse_snaps_to_its_quadrants() {
        let points = snap_points(&[shape(ShapeKind::Ellipse, 0.0, 0.0, 40.0, 20.0)], &[], &[]);
        assert_eq!(
            points,
            vec![
                WorldPoint::new(20.0, 0.0),
                WorldPoint::new(40.0, 10.0),
                WorldPoint::new(20.0, 20.0),
                WorldPoint::new(0.0, 10.0),
            ]
        );
    }
}
