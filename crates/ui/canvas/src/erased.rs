use crate::model::{Stroke, StrokeTool, WorldPoint};

struct Reach<'stroke> {
    stroke: &'stroke Stroke,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl<'stroke> Reach<'stroke> {
    fn new(stroke: &'stroke Stroke) -> Self {
        let half = stroke.size / 2.0;
        let mut reach = Self {
            stroke,
            left: f64::INFINITY,
            top: f64::INFINITY,
            right: f64::NEG_INFINITY,
            bottom: f64::NEG_INFINITY,
        };
        for point in &stroke.points {
            reach.left = reach.left.min(point.x - half);
            reach.top = reach.top.min(point.y - half);
            reach.right = reach.right.max(point.x + half);
            reach.bottom = reach.bottom.max(point.y + half);
        }
        reach
    }

    fn covers(&self, dot: WorldPoint) -> bool {
        (self.left..=self.right).contains(&dot.x)
            && (self.top..=self.bottom).contains(&dot.y)
            && self
                .stroke
                .points
                .iter()
                .zip(self.stroke.points.iter().skip(1))
                .any(|(start, end)| segment_distance(dot, *start, *end) <= self.stroke.size / 2.0)
    }
}

fn segment_distance(point: WorldPoint, start: WorldPoint, end: WorldPoint) -> f64 {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let length_squared = dx.mul_add(dx, dy * dy);
    if length_squared == 0.0 {
        return point.distance(start);
    }
    let along = ((point.x - start.x).mul_add(dx, (point.y - start.y) * dy) / length_squared)
        .clamp(0.0, 1.0);
    point.distance(WorldPoint::new(
        along.mul_add(dx, start.x),
        along.mul_add(dy, start.y),
    ))
}

pub(crate) fn erased_dots<'stroke>(
    strokes: impl IntoIterator<Item = &'stroke Stroke>,
    dots: &[WorldPoint],
) -> Vec<WorldPoint> {
    let reaches: Vec<Reach<'_>> = strokes
        .into_iter()
        .filter(|stroke| stroke.points.len() >= 2)
        .map(Reach::new)
        .collect();
    if reaches
        .iter()
        .all(|reach| reach.stroke.tool == StrokeTool::Pen)
    {
        return Vec::new();
    }
    dots.iter()
        .copied()
        .filter(|dot| {
            reaches
                .iter()
                .rev()
                .find(|reach| reach.covers(*dot))
                .is_some_and(|reach| reach.stroke.tool == StrokeTool::Eraser)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::INK_COLOR;

    fn stroke(tool: StrokeTool, size: f64, points: &[(f64, f64)]) -> Stroke {
        Stroke {
            points: points
                .iter()
                .map(|(x, y)| WorldPoint::new(*x, *y))
                .collect(),
            color: INK_COLOR.to_owned(),
            size,
            tool,
        }
    }

    #[test]
    fn only_dots_whose_topmost_stroke_is_an_eraser_come_back() {
        let strokes = [
            stroke(StrokeTool::Pen, 4.0, &[(0.0, 40.0), (200.0, 40.0)]),
            stroke(StrokeTool::Eraser, 20.0, &[(40.0, 0.0), (40.0, 200.0)]),
            stroke(StrokeTool::Pen, 4.0, &[(0.0, 120.0), (200.0, 120.0)]),
        ];
        let dots = [
            WorldPoint::new(40.0, 40.0),
            WorldPoint::new(40.0, 80.0),
            WorldPoint::new(40.0, 120.0),
            WorldPoint::new(80.0, 80.0),
        ];
        assert_eq!(
            erased_dots(&strokes, &dots),
            vec![WorldPoint::new(40.0, 40.0), WorldPoint::new(40.0, 80.0)]
        );
        assert!(erased_dots(&strokes[..1], &dots).is_empty());
    }
}
