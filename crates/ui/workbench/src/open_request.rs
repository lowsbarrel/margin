use margin_kernel::RelativePath;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OpenTarget {
    #[default]
    ActivePane,
    NewTab,
    SplitRight,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenRequest {
    pub path: RelativePath,
    pub target: OpenTarget,
    pub anchor: Option<String>,
}

impl OpenRequest {
    #[must_use]
    pub const fn new(path: RelativePath) -> Self {
        Self {
            path,
            target: OpenTarget::ActivePane,
            anchor: None,
        }
    }

    #[must_use]
    pub const fn in_target(mut self, target: OpenTarget) -> Self {
        self.target = target;
        self
    }

    #[must_use]
    pub fn at_anchor(mut self, anchor: impl Into<String>) -> Self {
        self.anchor = Some(anchor.into());
        self
    }
}
