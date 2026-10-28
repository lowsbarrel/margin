use std::fmt;

use crate::{RelativePath, StoreError};

pub type ChangeSink = Box<dyn Fn(Vec<RelativePath>) + Send + Sync>;

pub trait Watcher: Send + Sync {
    fn watch(&self, sink: ChangeSink) -> Result<Subscription, StoreError>;
}

pub struct Subscription {
    guard: Box<dyn Send>,
}

impl Subscription {
    #[must_use]
    pub fn new(guard: impl Send + 'static) -> Self {
        Self {
            guard: Box::new(guard),
        }
    }

    pub fn cancel(self) {
        drop(self.guard);
    }
}

impl fmt::Debug for Subscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Subscription")
    }
}
