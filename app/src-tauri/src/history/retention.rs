use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub(super) const HOUR_MS: u64 = 60 * 60 * 1000;
pub(super) const DAY_MS: u64 = 24 * HOUR_MS;
pub(super) const MONTH_MS: u64 = 30 * DAY_MS;
pub(super) const WEEK_MS: u64 = 7 * DAY_MS;

pub(super) const RETENTION_HARD_CAP: usize = 100;

// Earlier builds named snapshots with unix seconds, an order of magnitude below the millisecond range, so the magnitude tells the two apart.
pub(super) fn snapshot_millis(filename: &str) -> Option<u64> {
    const MILLIS_THRESHOLD: u64 = 100_000_000_000;
    let stem = filename.split('.').next()?;
    let timestamp: u64 = stem.parse().ok()?;
    Some(if timestamp < MILLIS_THRESHOLD {
        timestamp * 1000
    } else {
        timestamp
    })
}

pub(super) fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(super) fn snapshot_files(dir: &Path) -> Vec<(u64, String)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            snapshot_millis(&name).map(|ts| (ts, name))
        })
        .collect()
}

pub(super) fn retained_indices(stamps: &[u64], now_ms: u64) -> Vec<usize> {
    let mut keep: Vec<usize> = Vec::new();
    let mut buckets: [HashMap<u64, usize>; 3] = Default::default();

    for (i, &ts) in stamps.iter().enumerate() {
        let age = now_ms.saturating_sub(ts);
        if age < HOUR_MS {
            keep.push(i);
            continue;
        }
        let (slot, width) = if age < DAY_MS {
            (0, HOUR_MS)
        } else if age < MONTH_MS {
            (1, DAY_MS)
        } else {
            (2, WEEK_MS)
        };
        let bucket = buckets[slot].entry(ts / width).or_insert(i);
        if ts > stamps[*bucket] {
            *bucket = i;
        }
    }
    for bucket in buckets {
        keep.extend(bucket.into_values());
    }

    keep.sort_by_key(|&i| stamps[i]);
    if keep.len() > RETENTION_HARD_CAP {
        keep.drain(..keep.len() - RETENTION_HARD_CAP);
    }
    keep.sort_unstable();
    keep
}

pub(super) fn prune_snapshots(dir: &Path) {
    prune_snapshots_at(dir, now_millis())
}

pub(super) fn prune_snapshots_at(dir: &Path, now_ms: u64) {
    let files = snapshot_files(dir);
    let stamps: Vec<u64> = files.iter().map(|(ts, _)| *ts).collect();
    let kept = retained_indices(&stamps, now_ms);
    for (i, (_, name)) in files.into_iter().enumerate() {
        if !kept.contains(&i) {
            let _ = fs::remove_file(dir.join(name));
        }
    }
}
