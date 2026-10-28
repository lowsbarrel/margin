use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct Svg {
    pub bytes: Arc<[u8]>,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}
