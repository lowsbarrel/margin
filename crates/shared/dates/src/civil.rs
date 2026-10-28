use margin_kernel::UnixMillis;

use crate::utc_offset::UtcOffset;

const MILLIS_PER_DAY: i64 = 86_400_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl CivilDate {
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        let valid = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
        valid.then_some(Self { year, month, day })
    }

    #[must_use]
    pub fn days_since_epoch(self) -> i64 {
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year - era * 400;
        let month = i64::from(self.month);
        let day_of_year =
            (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    #[must_use]
    pub fn from_days_since_epoch(days: i64) -> Self {
        let shifted = days + 719_468;
        let era = shifted.div_euclid(146_097);
        let day_of_era = shifted - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_index = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_index + 2) / 5 + 1;
        let month = if month_index < 10 {
            month_index + 3
        } else {
            month_index - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Self {
            year: i32::try_from(year).unwrap_or(i32::MAX),
            month: u8::try_from(month).unwrap_or(1),
            day: u8::try_from(day).unwrap_or(1),
        }
    }

    #[must_use]
    pub fn to_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
}

impl CivilDateTime {
    #[must_use]
    pub fn from_unix_millis(at: UnixMillis, offset: UtcOffset) -> Self {
        let local = i64::try_from(at.as_millis())
            .unwrap_or(i64::MAX)
            .saturating_add(offset.millis());
        let days = local.div_euclid(MILLIS_PER_DAY);
        let within_day = local.rem_euclid(MILLIS_PER_DAY);
        let date = CivilDate::from_days_since_epoch(days);
        let part = |divisor: i64, modulus: i64| within_day / divisor % modulus;
        Self {
            year: date.year,
            month: date.month,
            day: date.day,
            hour: u8::try_from(part(3_600_000, 24)).unwrap_or(0),
            minute: u8::try_from(part(60_000, 60)).unwrap_or(0),
            second: u8::try_from(part(1000, 60)).unwrap_or(0),
            millisecond: u16::try_from(part(1, 1000)).unwrap_or(0),
        }
    }

    #[must_use]
    pub fn to_unix_millis(self, offset: UtcOffset) -> UnixMillis {
        let days = self.date().days_since_epoch();
        let local = days * MILLIS_PER_DAY
            + i64::from(self.hour) * 3_600_000
            + i64::from(self.minute) * 60_000
            + i64::from(self.second) * 1000
            + i64::from(self.millisecond);
        UnixMillis::from_millis(u64::try_from(local - offset.millis()).unwrap_or(0))
    }

    #[must_use]
    pub const fn date(self) -> CivilDate {
        CivilDate {
            year: self.year,
            month: self.month,
            day: self.day,
        }
    }

    #[must_use]
    pub fn to_iso_date(self) -> String {
        self.date().to_iso()
    }

    #[must_use]
    pub fn to_iso_datetime(self, offset: UtcOffset) -> String {
        format!(
            "{}T{:02}:{:02}:{:02}{}",
            self.to_iso_date(),
            self.hour,
            self.minute,
            self.second,
            offset.to_iso()
        )
    }
}

pub(crate) const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_known_instants() {
        let epoch = CivilDateTime::from_unix_millis(UnixMillis::from_millis(0), UtcOffset::UTC);
        assert_eq!(
            epoch.to_iso_datetime(UtcOffset::UTC),
            "1970-01-01T00:00:00Z"
        );
        let instant = CivilDateTime::from_unix_millis(
            UnixMillis::from_millis(1_761_647_405_123),
            UtcOffset::UTC,
        );
        assert_eq!(
            instant.to_iso_datetime(UtcOffset::UTC),
            "2025-10-28T10:30:05Z"
        );
        assert_eq!(instant.millisecond, 123);
    }

    #[test]
    fn applies_offsets_across_midnight() {
        let late = UnixMillis::from_millis(1_761_694_200_000);
        let rome = UtcOffset::from_minutes(60);
        let local = CivilDateTime::from_unix_millis(late, rome);
        assert_eq!(local.to_iso_datetime(rome), "2025-10-29T00:30:00+01:00");
        assert_eq!(local.to_unix_millis(rome), late);
    }

    #[test]
    fn round_trips_every_day_across_leap_years() {
        for days in -800_000..800_000_i64 {
            if days % 997 == 0 {
                assert_eq!(
                    CivilDate::from_days_since_epoch(days).days_since_epoch(),
                    days
                );
            }
        }
        assert_eq!(
            CivilDate::new(2024, 2, 29)
                .map(CivilDate::to_iso)
                .as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(CivilDate::new(2023, 2, 29), None);
        assert_eq!(CivilDate::new(2000, 2, 29).map(|date| date.day), Some(29));
        assert_eq!(CivilDate::new(1900, 2, 29), None);
    }
}
