use crate::civil::{CivilDate, CivilDateTime};
use crate::utc_offset::UtcOffset;

#[must_use]
pub fn parse_iso_date(text: &str) -> Option<CivilDate> {
    let mut parts = text.trim().splitn(3, '-');
    let year = parts.next().filter(|part| part.len() == 4)?.parse().ok()?;
    let month = parse_two_digits(parts.next()?)?;
    let day = parse_two_digits(parts.next()?)?;
    CivilDate::new(year, month, day)
}

#[must_use]
pub fn parse_iso_datetime(text: &str) -> Option<(CivilDateTime, Option<UtcOffset>)> {
    let text = text.trim();
    let (date_part, time_part) = text.split_once(['T', ' '])?;
    let date = parse_iso_date(date_part)?;
    let (clock, offset) = split_offset(time_part)?;
    let mut fields = clock.splitn(3, ':');
    let hour = parse_two_digits(fields.next()?).filter(|hour| *hour < 24)?;
    let minute = parse_two_digits(fields.next()?).filter(|minute| *minute < 60)?;
    let (second, millisecond) = match fields.next() {
        Some(seconds) => parse_seconds(seconds)?,
        None => (0, 0),
    };
    let time = CivilDateTime {
        year: date.year,
        month: date.month,
        day: date.day,
        hour,
        minute,
        second,
        millisecond,
    };
    Some((time, offset))
}

fn split_offset(time: &str) -> Option<(&str, Option<UtcOffset>)> {
    if let Some(clock) = time.strip_suffix(['Z', 'z']) {
        return Some((clock, Some(UtcOffset::UTC)));
    }
    let Some(sign_index) = time.rfind(['+', '-']) else {
        return Some((time, None));
    };
    let (clock, offset) = time.split_at(sign_index);
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let digits = offset.get(1..)?.replace(':', "");
    if digits.len() != 4 {
        return None;
    }
    let hours: i32 = digits.get(..2)?.parse().ok()?;
    let minutes: i32 = digits.get(2..)?.parse().ok()?;
    Some((
        clock,
        Some(UtcOffset::from_minutes(sign * (hours * 60 + minutes))),
    ))
}

fn parse_seconds(text: &str) -> Option<(u8, u16)> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let second = parse_two_digits(whole).filter(|second| *second < 60)?;
    let digits: String = fraction.chars().take(3).collect();
    if !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let millisecond = if digits.is_empty() {
        0
    } else {
        format!("{digits:0<3}").parse().ok()?
    };
    Some((second, millisecond))
}

fn parse_two_digits(text: &str) -> Option<u8> {
    if text.len() != 2 || !text.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dates_strictly() {
        assert_eq!(
            parse_iso_date("2026-10-28")
                .map(CivilDate::to_iso)
                .as_deref(),
            Some("2026-10-28")
        );
        assert_eq!(parse_iso_date("2026-13-01"), None);
        assert_eq!(parse_iso_date("2026-1-01"), None);
        assert_eq!(parse_iso_date("26-10-28"), None);
    }

    #[test]
    fn parses_datetimes_with_and_without_offsets() {
        let (time, offset) =
            parse_iso_datetime("2026-10-28T10:30").expect("minutes precision parses");
        assert_eq!(
            (time.hour, time.minute, time.second, offset),
            (10, 30, 0, None)
        );
        let (time, offset) = parse_iso_datetime("2026-10-28T10:30:05.5Z").expect("UTC parses");
        assert_eq!(
            (time.second, time.millisecond, offset),
            (5, 500, Some(UtcOffset::UTC))
        );
        let (_, offset) =
            parse_iso_datetime("2026-10-28 10:30:05-05:30").expect("negative offsets parse");
        assert_eq!(offset, Some(UtcOffset::from_minutes(-330)));
        assert_eq!(parse_iso_datetime("2026-10-28T25:00"), None);
        assert_eq!(parse_iso_datetime("2026-10-28"), None);
    }
}
