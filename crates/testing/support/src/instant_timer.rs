use std::time::Duration;

use margin_kernel::{BoxFuture, Timer};
use parking_lot::Mutex;

#[derive(Debug, Default)]
pub struct InstantTimer {
    slept: Mutex<Vec<Duration>>,
}

impl InstantTimer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn slept(&self) -> Vec<Duration> {
        self.slept.lock().clone()
    }
}

impl Timer for InstantTimer {
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
        self.slept.lock().push(duration);
        Box::pin(async {})
    }
}
