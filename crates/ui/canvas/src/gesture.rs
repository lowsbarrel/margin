use crate::camera::Camera;
use crate::model::{Shape, Stroke, WorldPoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PressButton {
    Primary,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PressOutcome {
    Handled,
    EditText(WorldPoint),
}

#[derive(Clone, Debug)]
pub(crate) enum Gesture {
    Idle,
    Pan {
        grab: (f64, f64),
        start: Camera,
    },
    Draw(Stroke),
    Shape {
        shape: Shape,
        snap_start: Option<WorldPoint>,
        snap_end: Option<WorldPoint>,
    },
}
