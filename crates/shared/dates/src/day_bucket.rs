use margin_kernel::UnixMillis;

use crate::civil::{CivilDate, CivilDateTime};
use crate::utc_offset::UtcOffset;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DayBucket {
    Today,
    Yesterday,
    Earlier(CivilDate),
}

#[must_use]
pub fn day_bucket(at: UnixMillis, now: UnixMillis, offset: UtcOffset) -> DayBucket {
    match days_between(at, now, offset) {
        0 => DayBucket::Today,
        1 => DayBucket::Yesterday,
        _ => DayBucket::Earlier(CivilDateTime::from_unix_millis(at, offset).date()),
    }
}

#[must_use]
pub fn days_between(earlier: UnixMillis, later: UnixMillis, offset: UtcOffset) -> i64 {
    let day = |at: UnixMillis| {
        CivilDateTime::from_unix_millis(at, offset)
            .date()
            .days_since_epoch()
    };
    day(later) - day(earlier)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3_600_000;

    #[test]
    fn groups_by_local_calendar_day() {
        let now = UnixMillis::from_millis(1_761_647_405_000);
        let offset = UtcOffset::UTC;
        assert_eq!(day_bucket(now, now, offset), DayBucket::Today);
        assert_eq!(
            day_bucket(
                UnixMillis::from_millis(now.as_millis() - 24 * HOUR),
                now,
                offset
            ),
            DayBucket::Yesterday
        );
        let older = day_bucket(
            UnixMillis::from_millis(now.as_millis() - 72 * HOUR),
            now,
            offset,
        );
        assert_eq!(
            older,
            DayBucket::Earlier(CivilDate::new(2025, 10, 25).expect("valid date"))
        );
    }

    #[test]
    fn the_offset_moves_the_day_boundary() {
        let just_after_utc_midnight = UnixMillis::from_millis(1_761_609_600_000 + HOUR / 2);
        let evening_before = UnixMillis::from_millis(1_761_609_600_000 - HOUR);
        assert_eq!(
            days_between(evening_before, just_after_utc_midnight, UtcOffset::UTC),
            1
        );
        assert_eq!(
            days_between(
                evening_before,
                just_after_utc_midnight,
                UtcOffset::from_minutes(120)
            ),
            0
        );
    }
}
