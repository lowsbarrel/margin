use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub(crate) const HOUR_MS: u64 = 60 * 60 * 1000;
pub(crate) const DAY_MS: u64 = 24 * HOUR_MS;
pub(crate) const MONTH_MS: u64 = 30 * DAY_MS;
pub(crate) const WEEK_MS: u64 = 7 * DAY_MS;
pub(crate) const RETENTION_HARD_CAP: usize = 100;

const MILLIS_THRESHOLD: u64 = 100_000_000_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub filename: String,
    pub timestamp: u64,
    pub size: u64,
}

pub(crate) fn snapshot_millis(filename: &str) -> Option<u64> {
    let stem = filename.split('.').next()?;
    let timestamp: u64 = stem.parse().ok()?;
    Some(if timestamp < MILLIS_THRESHOLD {
        timestamp * 1000
    } else {
        timestamp
    })
}

pub(crate) fn retained_indices(stamps: &[u64], now_ms: u64) -> Vec<usize> {
    let mut keep: Vec<(usize, u64)> = Vec::new();
    let mut buckets: HashMap<(u8, u64), (usize, u64)> = HashMap::new();

    for (index, &stamp) in stamps.iter().enumerate() {
        let age = now_ms.saturating_sub(stamp);
        if age < HOUR_MS {
            keep.push((index, stamp));
            continue;
        }
        let (slot, width) = if age < DAY_MS {
            (0_u8, HOUR_MS)
        } else if age < MONTH_MS {
            (1_u8, DAY_MS)
        } else {
            (2_u8, WEEK_MS)
        };
        let entry = buckets
            .entry((slot, stamp / width))
            .or_insert((index, stamp));
        if stamp > entry.1 {
            *entry = (index, stamp);
        }
    }
    keep.extend(buckets.into_values());

    keep.sort_by_key(|&(_, stamp)| stamp);
    if keep.len() > RETENTION_HARD_CAP {
        keep.drain(..keep.len() - RETENTION_HARD_CAP);
    }
    keep.sort_unstable_by_key(|&(index, _)| index);
    keep.into_iter().map(|(index, _)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_millis_accepts_seconds_and_milliseconds() {
        assert_eq!(snapshot_millis("1700000000.md"), Some(1_700_000_000_000));
        assert_eq!(snapshot_millis("1700000000000.md"), Some(1_700_000_000_000));
        assert_eq!(snapshot_millis("note.md"), None);
    }

    #[test]
    fn retention_keeps_the_recent_hour_and_one_newest_per_wider_bucket() {
        let now = 1000 * WEEK_MS;
        let stamps = vec![
            now - 1_000,
            now - 59 * 60 * 1000,
            now - 2 * HOUR_MS + 20 * 60_000,
            now - 2 * HOUR_MS + 10 * 60_000,
            now - 2 * DAY_MS + 2 * HOUR_MS,
            now - 2 * DAY_MS + HOUR_MS,
            now - 40 * DAY_MS + 2 * HOUR_MS,
            now - 40 * DAY_MS + HOUR_MS,
        ];

        let kept: Vec<u64> = retained_indices(&stamps, now)
            .into_iter()
            .map(|index| stamps[index])
            .collect();

        assert_eq!(
            kept,
            vec![
                now - 1_000,
                now - 59 * 60 * 1000,
                now - 2 * HOUR_MS + 20 * 60_000,
                now - 2 * DAY_MS + 2 * HOUR_MS,
                now - 40 * DAY_MS + 2 * HOUR_MS
            ]
        );
    }

    #[test]
    fn retention_hard_caps_a_note_rewritten_all_hour() {
        let now = 1_000_000_000_000_u64;
        let stamps: Vec<u64> = (0..3 * 3600_u64)
            .map(|second| now - second * 1000)
            .collect();

        let kept = retained_indices(&stamps, now);
        assert_eq!(kept.len(), RETENTION_HARD_CAP);

        for &index in &kept {
            assert!(
                stamps[index] >= stamps[stamps.len() - RETENTION_HARD_CAP],
                "an old snapshot survived the cap"
            );
        }
    }
}
