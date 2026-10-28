#[derive(Clone, Debug, PartialEq)]
pub struct DiagramStyle {
    pub foreground: String,
    pub background: String,
    pub accent: String,
    pub muted: String,
    pub font_size: f32,
    pub dark: bool,
}
