use margin_kernel::RelativePath;

use crate::open_request::OpenRequest;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkbenchEvent {
    Open(OpenRequest),
    Renamed {
        from: RelativePath,
        to: RelativePath,
    },
    Deleted(RelativePath),
    Saved(RelativePath),
    TagSelected(String),
    TreeChanged,
    Reveal(RelativePath),
}
