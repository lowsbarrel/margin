use std::time::Duration;

use margin_kernel::{Clock, UnixMillis};
use parking_lot::Mutex;

#[derive(Debug)]
pub struct FixedClock {
    now: Mutex<UnixMillis>,
}

impl FixedClock {
    #[must_use]
    pub const fn new(now: UnixMillis) -> Self {
        Self {
            now: Mutex::new(now),
        }
    }

    pub fn set(&self, now: UnixMillis) {
        *self.now.lock() = now;
    }

    pub fn advance(&self, duration: Duration) {
        let mut now = self.now.lock();
        *now = now.saturating_add(duration);
    }
}

impl Default for FixedClock {
    fn default() -> Self {
        Self::new(UnixMillis::from_millis(1_700_000_000_000))
    }
}

impl Clock for FixedClock {
    fn now(&self) -> UnixMillis {
        *self.now.lock()
    }
}
