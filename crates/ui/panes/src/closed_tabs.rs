use std::collections::VecDeque;

use margin_kernel::RelativePath;

const MAX_CLOSED_TABS: usize = 10;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClosedTabs(VecDeque<RelativePath>);

impl ClosedTabs {
    pub fn push(&mut self, path: RelativePath) {
        self.0.retain(|closed| *closed != path);
        self.0.push_front(path);
        self.0.truncate(MAX_CLOSED_TABS);
    }

    pub fn take(&mut self) -> Option<RelativePath> {
        self.0.pop_front()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn forget(&mut self, gone: impl Fn(&RelativePath) -> bool) {
        self.0.retain(|closed| !gone(closed));
    }

    pub fn rename(&mut self, remap: impl Fn(&RelativePath) -> Option<RelativePath>) {
        for closed in &mut self.0 {
            if let Some(next) = remap(closed) {
                *closed = next;
            }
        }
    }
}
