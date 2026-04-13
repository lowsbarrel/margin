import type { ManifestEntry, Manifest } from "./s3sync-manifest";

// ─── Types ───────────────────────────────────────────────────────────────

export type ChangeKind =
  | "upload"
  | "download"
  | "delete-remote"
  | "delete-local"
  | "conflict"
  | "conflict-delete-local"
  | "conflict-delete-remote";

export interface SyncAction {
  kind: ChangeKind;
  path: string;
}

// ─── 3-way diff ──────────────────────────────────────────────────────────

/** Soft-deleted entries are treated as "not present" for diff purposes */
function effectiveHash(
  map: Map<string, ManifestEntry>,
  path: string,
): string | null {
  const entry = map.get(path);
  if (!entry || entry.deleted_at) return null;
  return entry.hash;
}

export function computeSyncActions(
  base: Map<string, ManifestEntry>,
  local: Map<string, ManifestEntry>,
  remote: Map<string, ManifestEntry>,
): SyncAction[] {
  const allPaths = new Set([...base.keys(), ...local.keys(), ...remote.keys()]);
  const actions: SyncAction[] = [];

  for (const path of allPaths) {
    const baseHash = effectiveHash(base, path);
    const localHash = effectiveHash(local, path);
    const remoteHash = effectiveHash(remote, path);

    // Both sides agree → nothing to do
    if (localHash === remoteHash) continue;

    if (baseHash === null) {
      // File didn't exist at last sync
      if (localHash !== null && remoteHash === null) {
        actions.push({ kind: "upload", path });
      } else if (localHash === null && remoteHash !== null) {
        actions.push({ kind: "download", path });
      } else {
        // Both added with different content
        actions.push({ kind: "conflict", path });
      }
    } else {
      // File existed at last sync
      const localChanged = localHash !== baseHash;
      const remoteChanged = remoteHash !== baseHash;

      if (localHash === null && remoteHash === null) {
        continue; // both deleted
      } else if (localHash === null) {
        // Locally deleted
        actions.push({
          kind: remoteChanged ? "conflict-delete-local" : "delete-remote",
          path,
        });
      } else if (remoteHash === null) {
        // Remotely deleted
        actions.push({
          kind: localChanged ? "conflict-delete-remote" : "delete-local",
          path,
        });
      } else if (localChanged && !remoteChanged) {
        actions.push({ kind: "upload", path });
      } else if (!localChanged && remoteChanged) {
        actions.push({ kind: "download", path });
      } else {
        // Both changed differently
        actions.push({ kind: "conflict", path });
      }
    }
  }

  return actions;
}

// ─── Tombstone helpers ───────────────────────────────────────────────────

/** Entries older than 90 days are pruned from the manifest.
 *  90 days gives ample time for devices that go offline for extended periods
 *  to sync deletions without resurrecting deleted files. */
export const TOMBSTONE_TTL_SECONDS = 90 * 24 * 60 * 60;

export function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

export function collectTombstones(manifest: Manifest): Map<string, ManifestEntry> {
  const map = new Map<string, ManifestEntry>();
  for (const e of manifest.files) {
    if (e.deleted_at) map.set(e.path, e);
  }
  return map;
}

export function mergeTombstones(
  a: Map<string, ManifestEntry>,
  b: Map<string, ManifestEntry>,
): Map<string, ManifestEntry> {
  const merged = new Map(a);
  for (const [path, entry] of b) {
    const existing = merged.get(path);
    if (!existing || (entry.deleted_at ?? 0) > (existing.deleted_at ?? 0)) {
      merged.set(path, entry);
    }
  }
  return merged;
}

export function pruneTombstones(
  tombstones: Map<string, ManifestEntry>,
): ManifestEntry[] {
  const cutoff = nowSeconds() - TOMBSTONE_TTL_SECONDS;
  return Array.from(tombstones.values()).filter(
    (t) => (t.deleted_at ?? 0) > cutoff,
  );
}
