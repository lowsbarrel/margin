use std::path::PathBuf;

use chrono::{Local, Offset, TimeZone};
use gpui_kit::{App, SharedString, Window};
use margin_dates::{CivilDateTime, UtcOffset};
use margin_i18n::{t, t_count, t_with};
use margin_kernel::{RelativePath, UnixMillis};
use margin_workbench::{CurrentVault, Platform, notify_error};

const SIZE_UNITS: [&str; 4] = [
    "file_info_size_kb",
    "file_info_size_mb",
    "file_info_size_gb",
    "file_info_size_tb",
];

fn absolute(path: &RelativePath, cx: &App) -> PathBuf {
    CurrentVault::expect(cx).root.join(path.as_str())
}

pub(crate) fn open_external(path: &RelativePath, window: &mut Window, cx: &mut App) {
    let target = absolute(path, cx);
    if let Err(error) = Platform::open_external(cx, &target.to_string_lossy()) {
        notify_error(window, cx, "toast_cannot_open_file", &error);
    }
}

pub(crate) fn reveal(path: &RelativePath, window: &mut Window, cx: &mut App) {
    let target = absolute(path, cx);
    if let Err(error) = Platform::reveal(cx, &target) {
        notify_error(window, cx, "toast_open_finder_failed", &error);
    }
}

#[must_use]
pub(crate) fn format_size(bytes: u64, cx: &App) -> SharedString {
    match scaled_size(bytes) {
        None => t_count(cx, "file_info_size_bytes", bytes, &[]),
        Some((size, unit)) => t_with(cx, SIZE_UNITS[unit], &[("size", size.as_str())]),
    }
}

#[expect(clippy::cast_precision_loss, reason = "display rounding only")]
fn scaled_size(bytes: u64) -> Option<(String, usize)> {
    if bytes < 1024 {
        return None;
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < SIZE_UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    let size = if value < 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    };
    Some((size, unit))
}

#[must_use]
pub(crate) fn modified_label(at: UnixMillis, cx: &App) -> SharedString {
    let millis = i64::try_from(at.as_millis()).unwrap_or(i64::MAX);
    let offset = Local
        .timestamp_millis_opt(millis)
        .single()
        .map_or(UtcOffset::UTC, |time| {
            UtcOffset::from_minutes(time.offset().fix().local_minus_utc() / 60)
        });
    let time = CivilDateTime::from_unix_millis(at, offset);
    let month = t(cx, &format!("panels_month_{}", time.month));
    let clock = format!("{:02}:{:02}", time.hour, time.minute);
    t_with(
        cx,
        "file_info_modified_at",
        &[
            ("month", month.as_ref()),
            ("day", &time.day.to_string()),
            ("year", &time.year.to_string()),
            ("time", &clock),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_keep_one_decimal_below_ten_like_the_original() {
        assert_eq!(scaled_size(512), None);
        assert_eq!(scaled_size(1024), Some(("1.0".to_owned(), 0)));
        assert_eq!(scaled_size(5321), Some(("5.2".to_owned(), 0)));
        assert_eq!(scaled_size(44_222), Some(("43".to_owned(), 0)));
        assert_eq!(scaled_size(3 * 1024 * 1024), Some(("3.0".to_owned(), 1)));
        assert_eq!(scaled_size(200 * 1024 * 1024), Some(("200".to_owned(), 1)));
    }
}
