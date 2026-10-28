#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UtcOffset {
    minutes: i32,
}

impl UtcOffset {
    pub const UTC: Self = Self { minutes: 0 };

    #[must_use]
    pub const fn from_minutes(minutes: i32) -> Self {
        Self { minutes }
    }

    #[must_use]
    pub const fn minutes(self) -> i32 {
        self.minutes
    }

    #[must_use]
    pub(crate) fn millis(self) -> i64 {
        i64::from(self.minutes) * 60_000
    }

    #[must_use]
    pub fn to_iso(self) -> String {
        if self.minutes == 0 {
            return String::from("Z");
        }
        let sign = if self.minutes < 0 { '-' } else { '+' };
        let magnitude = self.minutes.unsigned_abs();
        format!("{sign}{:02}:{:02}", magnitude / 60, magnitude % 60)
    }
}
