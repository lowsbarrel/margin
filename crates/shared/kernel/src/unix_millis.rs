use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct UnixMillis(u64);

impl UnixMillis {
    #[must_use]
    pub const fn from_millis(millis: u64) -> Self {
        Self(millis)
    }

    #[must_use]
    pub const fn from_seconds(seconds: u64) -> Self {
        Self(seconds.saturating_mul(1000))
    }

    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn as_seconds(self) -> u64 {
        self.0 / 1000
    }

    #[must_use]
    pub fn saturating_sub(self, duration: Duration) -> Self {
        let millis = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self(self.0.saturating_sub(millis))
    }

    #[must_use]
    pub fn saturating_add(self, duration: Duration) -> Self {
        let millis = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(millis))
    }

    #[must_use]
    pub fn elapsed_since(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }
}

impl fmt::Display for UnixMillis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_units_without_overflow() {
        assert_eq!(UnixMillis::from_seconds(3).as_millis(), 3000);
        assert_eq!(UnixMillis::from_millis(3999).as_seconds(), 3);
        assert_eq!(UnixMillis::from_seconds(u64::MAX).as_millis(), u64::MAX);
    }

    #[test]
    fn arithmetic_saturates() {
        let start = UnixMillis::from_millis(500);
        assert_eq!(
            start.saturating_sub(Duration::from_secs(1)),
            UnixMillis::from_millis(0)
        );
        assert_eq!(
            UnixMillis::from_millis(2500).elapsed_since(start),
            Duration::from_secs(2)
        );
        assert_eq!(
            start.elapsed_since(UnixMillis::from_millis(900)),
            Duration::ZERO
        );
    }
}
