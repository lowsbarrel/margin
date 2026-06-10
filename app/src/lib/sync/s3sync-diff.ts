// ─── Sync timing helper ──────────────────────────────────────────────────
//
// The 3-way diff and tombstone merge/prune algorithm lives in Rust
// (compute_sync_actions / collect_tombstones / merge_tombstones /
// prune_tombstones in src-tauri/src/sync.rs). The previous TypeScript copies
// were dead code and were removed to avoid the two implementations drifting.
// Only this clock helper remains and is consumed by s3sync.ts.

export function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}
