use chrono::{Local, Offset, TimeZone};
use gpui_kit::{App, SharedString};
use margin_dates::{CivilDate, CivilDateTime, DayBucket, UtcOffset, day_bucket};
use margin_i18n::{t, t_count, t_with};
use margin_kernel::UnixMillis;
use margin_trash::PURGE_AGE;

const SECONDS_PER_MINUTE: u64 = 60;
const MINUTES_PER_HOUR: u64 = 60;
const MINUTES_PER_DAY: u64 = 24 * MINUTES_PER_HOUR;
const SECONDS_PER_DAY: u64 = MINUTES_PER_DAY * SECONDS_PER_MINUTE;
const BYTE_UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Elapsed {
    JustNow,
    Minutes(u64),
    Hours(u64),
    Days(u64),
}

impl Elapsed {
    pub(crate) fn between(earlier: UnixMillis, now: UnixMillis) -> Self {
        let minutes = now.elapsed_since(earlier).as_secs() / SECONDS_PER_MINUTE;
        match minutes {
            0 => Self::JustNow,
            minutes if minutes < MINUTES_PER_HOUR => Self::Minutes(minutes),
            minutes if minutes < MINUTES_PER_DAY => Self::Hours(minutes / MINUTES_PER_HOUR),
            minutes => Self::Days(minutes / MINUTES_PER_DAY),
        }
    }

    pub(crate) fn label(self, cx: &App) -> SharedString {
        let (key, count) = match self {
            Self::JustNow => return t(cx, "panels_relative_just_now"),
            Self::Minutes(count) => ("panels_relative_minutes", count),
            Self::Hours(count) => ("panels_relative_hours", count),
            Self::Days(count) => ("panels_relative_days", count),
        };
        t_with(cx, key, &[("count", count.to_string().as_str())])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row<Group> {
    Group(Group),
    Item(usize),
}

pub(crate) fn grouped_rows<Group: Copy + PartialEq>(
    groups: impl IntoIterator<Item = Group>,
) -> Vec<Row<Group>> {
    let mut rows = Vec::new();
    let mut current = None;
    for (index, group) in groups.into_iter().enumerate() {
        if current != Some(group) {
            rows.push(Row::Group(group));
            current = Some(group);
        }
        rows.push(Row::Item(index));
    }
    rows
}

pub(crate) fn local_date(at: UnixMillis, offset: UtcOffset) -> CivilDate {
    CivilDateTime::from_unix_millis(at, offset).date()
}

pub(crate) fn day_label(
    bucket: DayBucket,
    now: UnixMillis,
    offset: UtcOffset,
    cx: &App,
) -> SharedString {
    let date = match bucket {
        DayBucket::Today => return t(cx, "panels_day_today"),
        DayBucket::Yesterday => return t(cx, "panels_day_yesterday"),
        DayBucket::Earlier(date) => date,
    };
    let key = if date.year == local_date(now, offset).year {
        "panels_date_day_month"
    } else {
        "panels_date_day_month_year"
    };
    date_label(date, key, MonthName::Long, "", cx)
}

pub(crate) fn full_date_label(date: CivilDate, cx: &App) -> SharedString {
    date_label(date, "panels_date_day_month_year", MonthName::Long, "", cx)
}

pub(crate) fn snapshot_label(
    at: UnixMillis,
    now: UnixMillis,
    offset: UtcOffset,
    cx: &App,
) -> SharedString {
    let time = time_label(at, offset);
    let date = match day_bucket(at, now, offset) {
        DayBucket::Today => return time,
        DayBucket::Yesterday => {
            return t_with(cx, "history_yesterday_at", &[("time", time.as_ref())]);
        }
        DayBucket::Earlier(date) => date,
    };
    let key = if date.year == local_date(now, offset).year {
        "history_date_time"
    } else {
        "history_date_year_time"
    };
    date_label(date, key, MonthName::Short, &time, cx)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MonthName {
    Long,
    Short,
}

fn date_label(
    date: CivilDate,
    key: &str,
    month_name: MonthName,
    time: &str,
    cx: &App,
) -> SharedString {
    let month_key = match month_name {
        MonthName::Long => format!("panels_month_{}", date.month),
        MonthName::Short => format!("panels_month_short_{}", date.month),
    };
    let month = t(cx, &month_key);
    let day = date.day.to_string();
    let year = date.year.to_string();
    let arguments = [
        ("month", month.as_ref()),
        ("day", day.as_str()),
        ("year", year.as_str()),
        ("time", time),
    ];
    t_with(cx, key, &arguments)
}

pub(crate) fn time_label(at: UnixMillis, offset: UtcOffset) -> SharedString {
    let time = CivilDateTime::from_unix_millis(at, offset);
    SharedString::from(format!("{:02}:{:02}", time.hour, time.minute))
}

pub(crate) fn days_until_purge(deleted_at: UnixMillis, now: UnixMillis) -> u64 {
    deleted_at
        .saturating_add(PURGE_AGE)
        .elapsed_since(now)
        .as_secs()
        / SECONDS_PER_DAY
}

pub(crate) fn purge_label(days: u64, cx: &App) -> SharedString {
    if days == 0 {
        t(cx, "trash_purge_today")
    } else {
        t_count(cx, "trash_purge_days", days, &[])
    }
}

pub(crate) fn local_offset(at: UnixMillis) -> UtcOffset {
    let millis = i64::try_from(at.as_millis()).unwrap_or(i64::MAX);
    Local
        .timestamp_millis_opt(millis)
        .single()
        .map_or(UtcOffset::UTC, |time| {
            UtcOffset::from_minutes(time.offset().fix().local_minus_utc() / 60)
        })
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a displayed size keeps one decimal at most"
)]
pub(crate) fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} {}", BYTE_UNITS[0]);
    }
    let mut unit = 0;
    let mut scaled = bytes as f64;
    while scaled >= 1024.0 && unit < BYTE_UNITS.len() - 1 {
        scaled /= 1024.0;
        unit += 1;
    }
    if scaled < 10.0 {
        format!("{scaled:.1} {}", BYTE_UNITS[unit])
    } else {
        format!("{scaled:.0} {}", BYTE_UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE_MS: u64 = 60_000;
    const HOUR_MS: u64 = 60 * MINUTE_MS;
    const DAY_MS: u64 = 24 * HOUR_MS;
    const NOW: UnixMillis = UnixMillis::from_millis(1_761_647_405_000);

    fn ago(millis: u64) -> UnixMillis {
        UnixMillis::from_millis(NOW.as_millis() - millis)
    }

    #[test]
    fn elapsed_time_rounds_down_to_the_largest_whole_unit() {
        assert_eq!(Elapsed::between(ago(59_999), NOW), Elapsed::JustNow);
        assert_eq!(
            Elapsed::between(ago(5 * MINUTE_MS), NOW),
            Elapsed::Minutes(5)
        );
        assert_eq!(
            Elapsed::between(ago(HOUR_MS - 1), NOW),
            Elapsed::Minutes(59)
        );
        assert_eq!(Elapsed::between(ago(3 * HOUR_MS), NOW), Elapsed::Hours(3));
        assert_eq!(Elapsed::between(ago(DAY_MS), NOW), Elapsed::Days(1));
        assert_eq!(
            Elapsed::between(UnixMillis::from_millis(NOW.as_millis() + HOUR_MS), NOW),
            Elapsed::JustNow
        );
    }

    #[test]
    fn rows_group_consecutive_items_under_their_local_day() {
        let stamps = [ago(HOUR_MS), ago(2 * HOUR_MS), ago(DAY_MS), ago(3 * DAY_MS)];
        let rows = grouped_rows(stamps.map(|at| day_bucket(at, NOW, UtcOffset::UTC)));
        assert_eq!(
            rows,
            vec![
                Row::Group(DayBucket::Today),
                Row::Item(0),
                Row::Item(1),
                Row::Group(DayBucket::Yesterday),
                Row::Item(2),
                Row::Group(DayBucket::Earlier(
                    CivilDate::new(2025, 10, 25).expect("valid date")
                )),
                Row::Item(3),
            ]
        );
    }

    #[test]
    fn purge_countdown_counts_whole_days_left_and_stops_at_zero() {
        assert_eq!(days_until_purge(NOW, NOW), 30);
        assert_eq!(days_until_purge(ago(DAY_MS / 2), NOW), 29);
        assert_eq!(days_until_purge(ago(29 * DAY_MS + 1), NOW), 0);
        assert_eq!(days_until_purge(ago(40 * DAY_MS), NOW), 0);
    }

    #[test]
    fn clock_times_are_zero_padded_in_the_given_offset() {
        let at = UnixMillis::from_millis(1_761_609_600_000 + 5 * MINUTE_MS);
        assert_eq!(time_label(at, UtcOffset::UTC).as_ref(), "00:05");
        assert_eq!(
            time_label(at, UtcOffset::from_minutes(90)).as_ref(),
            "01:35"
        );
    }

    #[test]
    fn byte_sizes_use_one_decimal_below_ten() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(20 * 1024 * 1024), "20 MB");
    }
}
