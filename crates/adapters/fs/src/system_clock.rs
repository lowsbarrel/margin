use std::time::{SystemTime, UNIX_EPOCH};

use margin_kernel::{Clock, UnixMillis};

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl SystemClock {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> UnixMillis {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
            |_| UnixMillis::default(),
            |elapsed| {
                UnixMillis::from_millis(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
            },
        )
    }
}
