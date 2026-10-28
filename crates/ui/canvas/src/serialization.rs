use std::fmt::Write;

use serde_json::{Map, Value};

use crate::camera::Camera;
use crate::model::{
    Drawing, INK_COLOR, Shape, ShapeKind, Stroke, StrokeTool, TextLabel, WorldPoint,
};

const DEFAULT_SIZE: f64 = 3.0;
const DEFAULT_FONT_SIZE: f64 = 16.0;

pub(crate) fn serialize(drawing: &Drawing, camera: Camera) -> String {
    let mut out = String::from("{\"strokes\":[");
    for (index, stroke) in drawing.strokes.iter().enumerate() {
        separator(&mut out, index);
        out.push_str("{\"points\":[");
        for (point_index, point) in stroke.points.iter().enumerate() {
            separator(&mut out, point_index);
            write_fields(&mut out, &[("x", point.x), ("y", point.y)]);
            out.push('}');
        }
        out.push_str("],\"color\":");
        write_string(&mut out, &stroke.color);
        out.push_str(",\"size\":");
        write_number(&mut out, stroke.size);
        out.push_str(match stroke.tool {
            StrokeTool::Pen => ",\"tool\":\"pen\"}",
            StrokeTool::Eraser => ",\"tool\":\"eraser\"}",
        });
    }
    out.push_str("],\"shapes\":[");
    for (index, shape) in drawing.shapes.iter().enumerate() {
        separator(&mut out, index);
        out.push_str("{\"kind\":");
        write_string(&mut out, shape.kind.as_str());
        out.push(',');
        out.push_str(&fields_tail(&[
            ("x1", shape.x1),
            ("y1", shape.y1),
            ("x2", shape.x2),
            ("y2", shape.y2),
        ]));
        out.push_str(",\"color\":");
        write_string(&mut out, &shape.color);
        out.push_str(",\"size\":");
        write_number(&mut out, shape.size);
        out.push('}');
    }
    out.push_str("],\"textLabels\":[");
    for (index, label) in drawing.labels.iter().enumerate() {
        separator(&mut out, index);
        write_fields(&mut out, &[("x", label.x), ("y", label.y)]);
        out.push_str(",\"text\":");
        write_string(&mut out, &label.text);
        out.push_str(",\"color\":");
        write_string(&mut out, &label.color);
        out.push_str(",\"fontSize\":");
        write_number(&mut out, label.font_size);
        out.push('}');
    }
    out.push_str("],");
    out.push_str(&fields_tail(&[
        ("camX", camera.x),
        ("camY", camera.y),
        ("zoom", camera.zoom),
    ]));
    out.push('}');
    out
}

pub(crate) fn deserialize(raw: &str) -> Option<(Drawing, Camera)> {
    if raw.is_empty() {
        return None;
    }
    let Ok(Value::Object(data)) = serde_json::from_str::<Value>(raw) else {
        return None;
    };
    let drawing = Drawing {
        strokes: items(&data, "strokes").filter_map(parse_stroke).collect(),
        shapes: items(&data, "shapes").filter_map(parse_shape).collect(),
        labels: items(&data, "textLabels").filter_map(parse_label).collect(),
    };
    let camera = Camera {
        x: number_or(data.get("camX"), 0.0),
        y: number_or(data.get("camY"), 0.0),
        zoom: number_or(data.get("zoom"), 1.0),
    };
    Some((drawing, camera))
}

fn items<'data>(
    data: &'data Map<String, Value>,
    key: &str,
) -> impl Iterator<Item = &'data Map<String, Value>> {
    data.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

fn number(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

fn number_or(value: Option<&Value>, fallback: f64) -> f64 {
    number(value).unwrap_or(fallback)
}

fn color(item: &Map<String, Value>) -> String {
    item.get("color")
        .and_then(Value::as_str)
        .unwrap_or(INK_COLOR)
        .to_owned()
}

fn parse_stroke(item: &Map<String, Value>) -> Option<Stroke> {
    let points: Vec<WorldPoint> = item
        .get("points")?
        .as_array()?
        .iter()
        .filter_map(|point| {
            Some(WorldPoint::new(
                number(point.get("x"))?,
                number(point.get("y"))?,
            ))
        })
        .collect();
    if points.is_empty() {
        return None;
    }
    let tool = if item.get("tool").and_then(Value::as_str) == Some("eraser") {
        StrokeTool::Eraser
    } else {
        StrokeTool::Pen
    };
    Some(Stroke {
        points,
        color: color(item),
        size: number_or(item.get("size"), DEFAULT_SIZE),
        tool,
    })
}

fn parse_shape(item: &Map<String, Value>) -> Option<Shape> {
    Some(Shape {
        kind: ShapeKind::parse(item.get("kind")?.as_str()?)?,
        x1: number(item.get("x1"))?,
        y1: number(item.get("y1"))?,
        x2: number(item.get("x2"))?,
        y2: number(item.get("y2"))?,
        color: color(item),
        size: number_or(item.get("size"), DEFAULT_SIZE),
    })
}

fn parse_label(item: &Map<String, Value>) -> Option<TextLabel> {
    Some(TextLabel {
        text: item.get("text")?.as_str()?.to_owned(),
        x: number(item.get("x"))?,
        y: number(item.get("y"))?,
        color: color(item),
        font_size: number_or(item.get("fontSize"), DEFAULT_FONT_SIZE),
    })
}

fn separator(out: &mut String, index: usize) {
    if index > 0 {
        out.push(',');
    }
}

fn write_fields(out: &mut String, fields: &[(&str, f64)]) {
    out.push('{');
    out.push_str(&fields_tail(fields));
}

fn fields_tail(fields: &[(&str, f64)]) -> String {
    let mut out = String::new();
    for (index, (name, value)) in fields.iter().enumerate() {
        separator(&mut out, index);
        write_string(&mut out, name);
        out.push(':');
        write_number(&mut out, *value);
    }
    out
}

fn write_string(out: &mut String, value: &str) {
    out.push_str(&serde_json::to_string(value).expect("strings always serialize to JSON"));
}

fn write_number(out: &mut String, value: f64) {
    let magnitude = value.abs();
    if value == 0.0 || !value.is_finite() {
        out.push('0');
    } else if (1e-6..1e21).contains(&magnitude) {
        write!(out, "{value}").expect("writing to a String never fails");
    } else {
        let scientific = format!("{value:e}");
        match scientific.split_once('e') {
            Some((mantissa, exponent)) if !exponent.starts_with('-') => {
                write!(out, "{mantissa}e+{exponent}").expect("writing to a String never fails");
            }
            _ => out.push_str(&scientific),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAURI_FIXTURE: &str = concat!(
        r#"{"strokes":[{"points":[{"x":10,"y":20.5},{"x":11.25,"y":-3}],"color":"ink","size":3,"tool":"pen"},"#,
        r##"{"points":[{"x":0.1,"y":0},{"x":1e-7,"y":1e+21}],"color":"#000","size":24,"tool":"eraser"}],"##,
        r##""shapes":[{"kind":"arrow","x1":0,"y1":0,"x2":100,"y2":50,"color":"#ef4444","size":1.5}],"##,
        r#""textLabels":[{"x":5,"y":6,"text":"Hello \"world\"\n","color":"ink","fontSize":16}],"#,
        r#""camX":-12.5,"camY":40,"zoom":1.2}"#
    );

    #[test]
    fn tauri_canvas_round_trips_byte_for_byte() {
        let (drawing, camera) = deserialize(TAURI_FIXTURE).expect("the fixture parses");
        assert_eq!(drawing.strokes.len(), 2);
        assert_eq!(drawing.strokes[1].tool, StrokeTool::Eraser);
        assert_eq!(drawing.shapes[0].kind, ShapeKind::Arrow);
        assert_eq!(drawing.labels[0].text, "Hello \"world\"\n");
        assert_eq!(serialize(&drawing, camera), TAURI_FIXTURE);
    }

    #[test]
    fn tolerates_malformed_entries_like_the_original() {
        let raw = r#"{"strokes":[{"points":[]},{"points":[{"x":1,"y":"a"},{"x":2,"y":3}]},7],
            "shapes":[{"kind":"star","x1":0,"y1":0,"x2":1,"y2":1},{"kind":"rect","x1":0,"y1":0,"x2":1}],
            "textLabels":[{"x":1,"y":2,"text":"a","fontSize":"big","extra":true}],"zoom":"x"}"#;
        let (drawing, camera) = deserialize(raw).expect("objects parse");
        assert_eq!(drawing.strokes.len(), 1);
        assert_eq!(drawing.strokes[0].points, vec![WorldPoint::new(2.0, 3.0)]);
        assert_eq!(drawing.strokes[0].color, INK_COLOR);
        assert!((drawing.strokes[0].size - DEFAULT_SIZE).abs() < f64::EPSILON);
        assert!(drawing.shapes.is_empty());
        assert!((drawing.labels[0].font_size - DEFAULT_FONT_SIZE).abs() < f64::EPSILON);
        assert_eq!(camera, Camera::default());
        assert!(deserialize("").is_none());
        assert!(deserialize("[1]").is_none());
        assert!(deserialize("{not json").is_none());
    }
}
