import {
  s3Configure,
  s3Upload,
  s3Download,
  s3Delete,
  type S3Config,
} from "$lib/s3/bridge";
import { encryptBlob, decryptBlob } from "$lib/crypto/bridge";
import {
  readFileBytes,
  writeFileBytes,
  walkDirectory,
  fileExists,
  createDirectory,
  deleteEntry,
  setMtime,
} from "$lib/fs/bridge";
import { editor } from "$lib/stores/editor.svelte";
import { files } from "$lib/stores/files.svelte";
import { toast } from "$lib/stores/toast.svelte";

// ─── Types ───────────────────────────────────────────────────────────────

interface ManifestEntry {
  path: string;
  hash: string;
  /** Seconds since UNIX epoch — actual file modification time */
  modified: number;
  /** Seconds since UNIX epoch — set when the file is soft-deleted */
  deleted_at?: number;
}

interface Manifest {
  version: number;
  files: ManifestEntry[];
}

/** Validate that a parsed object has the expected Manifest shape. */
function validateManifest(obj: unknown): Manifest {
  if (typeof obj !== "object" || obj === null)
    throw new Error("Manifest is not an object");
  const m = obj as Record<string, unknown>;
  if (typeof m.version !== "number")
    throw new Error("Manifest missing version");
  if (!Array.isArray(m.files)) throw new Error("Manifest missing files array");
  for (const entry of m.files) {
    if (typeof entry !== "object" || entry === null)
      throw new Error("Invalid manifest entry");
    const e = entry as Record<string, unknown>;
    if (typeof e.path !== "string")
      throw new Error("Manifest entry missing path");
    if (typeof e.hash !== "string")
      throw new Error("Manifest entry missing hash");
    if (typeof e.modified !== "number")
      throw new Error("Manifest entry missing modified");
  }
  return obj as Manifest;
}

export type ConflictStrategy = "local_wins" | "keep_newer";

export interface SyncOptions {
  conflictStrategy?: ConflictStrategy;
}

type ChangeKind =
  | "upload"
  | "download"
  | "delete-remote"
  | "delete-local"
  | "conflict"
  | "conflict-delete-local"
  | "conflict-delete-remote";

interface SyncAction {
  kind: ChangeKind;
  path: string;
}

// ─── Abort / state ───────────────────────────────────────────────────────

let activeSyncAbort: AbortController | null = null;
/** Resolves when no sync is in flight. Used as a mutex to prevent concurrent syncs. */
let syncLock: Promise<void> = Promise.resolve();

/** Cancel any in-flight sync operation */
export function cancelSync(): void {
  activeSyncAbort?.abort();
  activeSyncAbort = null;
}

export function isSyncing(): boolean {
  return activeSyncAbort !== null;
}

function checkAbort(signal: AbortSignal): void {
  if (signal.aborted) throw new Error("Sync cancelled");
}

// ─── Helpers ─────────────────────────────────────────────────────────────

async function sha256hex(data: Uint8Array): Promise<string> {
  const hash = await crypto.subtle.digest(
    "SHA-256",
    data.buffer as ArrayBuffer,
  );
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

interface LocalFile {
  path: string;
  fullPath: string;
  modified: number;
}

async function walkVault(basePath: string): Promise<LocalFile[]> {
  // Single IPC call for the whole tree — replaces recursive JS walk
  const all = await walkDirectory(basePath);
  return all
    .filter((e) => !e.is_dir)
    .map((e) => {
      const relativePath = e.path.slice(basePath.length + 1);
      return { path: relativePath, fullPath: e.path, modified: e.modified };
    });
}

let conflictCounter = 0;

function conflictCopyName(path: string): string {
  const now = new Date();
  const ts = [
    now.getFullYear(),
    String(now.getMonth() + 1).padStart(2, "0"),
    String(now.getDate()).padStart(2, "0"),
    "-",
    String(now.getHours()).padStart(2, "0"),
    String(now.getMinutes()).padStart(2, "0"),
    String(now.getSeconds()).padStart(2, "0"),
  ].join("");
  // Append a monotonic counter to guarantee uniqueness within the same second
  const seq = ++conflictCounter;
  const suffix = `sync-conflict-${ts}-${seq}`;
  const dot = path.lastIndexOf(".");
  if (dot > 0) return `${path.slice(0, dot)}.${suffix}${path.slice(dot)}`;
  return `${path}.${suffix}`;
}

async function ensureParentDir(
  vaultPath: string,
  relativePath: string,
): Promise<void> {
  const parts = relativePath.split("/");
  if (parts.length > 1) {
    await createDirectory(`${vaultPath}/${parts.slice(0, -1).join("/")}`);
  }
}

// ─── Base manifest (local) ──────────────────────────────────────────────

const BASE_MANIFEST_FILE = "sync-base.enc";

async function loadBaseManifest(
  vaultPath: string,
  encryptionKey: number[],
): Promise<Manifest> {
  const path = `${vaultPath}/.margin/${BASE_MANIFEST_FILE}`;
  try {
    if (!(await fileExists(path))) return { version: 2, files: [] };
    const enc = await readFileBytes(path);
    const dec = await decryptBlob(enc, encryptionKey);
    return validateManifest(JSON.parse(new TextDecoder().decode(dec)));
  } catch {
    return { version: 2, files: [] };
  }
}

async function saveBaseManifest(
  vaultPath: string,
  encryptionKey: number[],
  manifest: Manifest,
): Promise<void> {
  const json = new TextEncoder().encode(JSON.stringify(manifest));
  const enc = await encryptBlob(json, encryptionKey);
  await createDirectory(`${vaultPath}/.margin`);
  await writeFileBytes(`${vaultPath}/.margin/${BASE_MANIFEST_FILE}`, enc);
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

function computeSyncActions(
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
const TOMBSTONE_TTL_SECONDS = 90 * 24 * 60 * 60;

function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

function collectTombstones(manifest: Manifest): Map<string, ManifestEntry> {
  const map = new Map<string, ManifestEntry>();
  for (const e of manifest.files) {
    if (e.deleted_at) map.set(e.path, e);
  }
  return map;
}

function mergeTombstones(
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

function pruneTombstones(
  tombstones: Map<string, ManifestEntry>,
): ManifestEntry[] {
  const cutoff = nowSeconds() - TOMBSTONE_TTL_SECONDS;
  return Array.from(tombstones.values()).filter(
    (t) => (t.deleted_at ?? 0) > cutoff,
  );
}

// ─── Main sync ───────────────────────────────────────────────────────────

/** Read a local file (from cache or disk) and encrypt it for S3. */
async function readAndEncrypt(
  path: string,
  vaultPath: string,
  encryptionKey: number[],
  cache: Map<string, Uint8Array>,
): Promise<Uint8Array> {
  const data = cache.get(path) ?? (await readFileBytes(`${vaultPath}/${path}`));
  return encryptBlob(data, encryptionKey);
}

/** Download a file from S3, decrypt it, and write it to disk. Returns the decrypted data. */
async function downloadAndDecrypt(
  s3Key: string,
  destPath: string,
  encryptionKey: number[],
  vaultPath: string,
  relativePath: string,
): Promise<Uint8Array> {
  const encrypted = await s3Download(s3Key);
  const decrypted = await decryptBlob(encrypted, encryptionKey);
  await ensureParentDir(vaultPath, relativePath);
  await writeFileBytes(destPath, decrypted);
  return decrypted;
}

/** Record a file as deleted in tombstones and remove it from mergedFiles. */
function markTombstone(
  path: string,
  tombstones: Map<string, ManifestEntry>,
  mergedFiles: Map<string, ManifestEntry>,
  fallbackMaps: Map<string, ManifestEntry>[],
): void {
  const existing =
    mergedFiles.get(path) ??
    fallbackMaps.reduce<ManifestEntry | undefined>(
      (acc, m) => acc ?? m.get(path),
      undefined,
    );
  if (existing) {
    tombstones.set(path, { ...existing, deleted_at: nowSeconds() });
  }
  mergedFiles.delete(path);
}

export function syncToS3(
  vaultPath: string,
  vaultId: string,
  encryptionKey: number[],
  s3Config: S3Config,
  options?: SyncOptions,
): Promise<void> {
  // Chain onto the sync lock so only one sync runs at a time.
  // If a sync is already in flight it finishes first, then this one starts.
  const ticket = syncLock.then(() =>
    doSyncToS3(vaultPath, vaultId, encryptionKey, s3Config, options),
  );
  syncLock = ticket.catch(() => {}); // swallow so the chain never rejects
  return ticket;
}

async function doSyncToS3(
  vaultPath: string,
  vaultId: string,
  encryptionKey: number[],
  s3Config: S3Config,
  options?: SyncOptions,
): Promise<void> {
  cancelSync();

  const abortController = new AbortController();
  activeSyncAbort = abortController;
  const { signal } = abortController;
  const conflictStrategy: ConflictStrategy =
    options?.conflictStrategy ?? "local_wins";

  editor.setSyncStatus("syncing");

  try {
    await s3Configure(s3Config);
    const s3Prefix = `${vaultId}/`;

    // 1. Load base manifest (last synced state)
    checkAbort(signal);
    const baseManifest = await loadBaseManifest(vaultPath, encryptionKey);
    const baseMap = new Map(baseManifest.files.map((e) => [e.path, e]));

    // 2. Build current local manifest
    //    Optimisation: skip hashing files whose mtime matches the base entry
    const localFiles = await walkVault(vaultPath);
    checkAbort(signal);

    const localManifest: Manifest = { version: 2, files: [] };
    const fileDataCache = new Map<string, Uint8Array>();

    for (const file of localFiles) {
      checkAbort(signal);
      const baseEntry = baseMap.get(file.path);
      if (
        baseEntry &&
        !baseEntry.deleted_at &&
        baseEntry.modified === file.modified
      ) {
        // mtime unchanged & not a tombstone — reuse previous hash without reading file
        localManifest.files.push(baseEntry);
      } else {
        const data = await readFileBytes(file.fullPath);
        const hash = await sha256hex(data);
        fileDataCache.set(file.path, data);
        localManifest.files.push({
          path: file.path,
          hash,
          modified: file.modified,
        });
      }
    }

    // 3. Download remote manifest
    let remoteManifest: Manifest = { version: 2, files: [] };
    try {
      checkAbort(signal);
      const encManifest = await s3Download(`${s3Prefix}manifest.enc`);
      const decManifest = await decryptBlob(encManifest, encryptionKey);
      remoteManifest = validateManifest(
        JSON.parse(new TextDecoder().decode(decManifest)),
      );
    } catch (err) {
      if (signal.aborted) throw new Error("Sync cancelled");
      // First sync — no remote manifest yet
    }

    // 4. Build maps & compute 3-way diff
    const localMap = new Map(localManifest.files.map((e) => [e.path, e]));
    const remoteMap = new Map(remoteManifest.files.map((e) => [e.path, e]));
    const actions = computeSyncActions(baseMap, localMap, remoteMap);
    checkAbort(signal);

    // 5. Execute each action
    const conflicts: string[] = [];
    const mergedFiles = new Map<string, ManifestEntry>();
    let actionsDone = 0;
    const actionsTotal = actions.length;
    editor.setSyncProgress(
      actionsTotal > 0 ? { total: actionsTotal, done: 0 } : null,
    );

    // Collect tombstones from both sides and merge them
    const tombstones = mergeTombstones(
      collectTombstones(baseManifest),
      collectTombstones(remoteManifest),
    );

    // Start with all local files in the merged result
    for (const entry of localManifest.files) mergedFiles.set(entry.path, entry);

    for (const action of actions) {
      checkAbort(signal);

      switch (action.kind) {
        // ── Local add / modify → push to S3 ──────────────────────
        case "upload": {
          const encrypted = await readAndEncrypt(
            action.path,
            vaultPath,
            encryptionKey,
            fileDataCache,
          );
          await s3Upload(`${s3Prefix}files/${action.path}.enc`, encrypted);
          tombstones.delete(action.path);
          break;
        }

        // ── Remote add / modify → pull from S3 ───────────────────
        case "download": {
          await downloadAndDecrypt(
            `${s3Prefix}files/${action.path}.enc`,
            `${vaultPath}/${action.path}`,
            encryptionKey,
            vaultPath,
            action.path,
          );
          const remoteEntry = remoteMap.get(action.path)!;
          await setMtime(`${vaultPath}/${action.path}`, remoteEntry.modified);
          mergedFiles.set(action.path, {
            path: remoteEntry.path,
            hash: remoteEntry.hash,
            modified: remoteEntry.modified,
          });
          tombstones.delete(action.path);
          break;
        }

        // ── Locally deleted → delete from S3 ─────────────────────
        case "delete-remote": {
          try {
            await s3Delete(`${s3Prefix}files/${action.path}.enc`);
          } catch {
            /* already gone */
          }
          markTombstone(action.path, tombstones, mergedFiles, [baseMap]);
          break;
        }

        // ── Remotely deleted → delete locally ────────────────────
        case "delete-local": {
          try {
            await deleteEntry(`${vaultPath}/${action.path}`);
          } catch {
            /* already gone */
          }
          markTombstone(action.path, tombstones, mergedFiles, [remoteMap]);
          break;
        }

        // ── Conflict: both sides modified ────────────────────────
        case "conflict": {
          const localEntry = localMap.get(action.path)!;
          const remoteEntry = remoteMap.get(action.path)!;

          const remoteWins =
            conflictStrategy === "keep_newer" &&
            remoteEntry.modified > localEntry.modified;

          if (remoteWins) {
            // Remote is newer → keep remote, save local as conflict copy
            const localData =
              fileDataCache.get(action.path) ??
              (await readFileBytes(`${vaultPath}/${action.path}`));
            const conflictPath = conflictCopyName(action.path);
            await ensureParentDir(vaultPath, conflictPath);
            await writeFileBytes(`${vaultPath}/${conflictPath}`, localData);

            await downloadAndDecrypt(
              `${s3Prefix}files/${action.path}.enc`,
              `${vaultPath}/${action.path}`,
              encryptionKey,
              vaultPath,
              action.path,
            );
            await setMtime(`${vaultPath}/${action.path}`, remoteEntry.modified);

            mergedFiles.set(action.path, {
              path: remoteEntry.path,
              hash: remoteEntry.hash,
              modified: remoteEntry.modified,
            });
          } else {
            // Local wins (default) → keep local, save remote as conflict copy
            const encrypted = await s3Download(
              `${s3Prefix}files/${action.path}.enc`,
            );
            const decrypted = await decryptBlob(encrypted, encryptionKey);
            const conflictPath = conflictCopyName(action.path);
            await ensureParentDir(vaultPath, conflictPath);
            await writeFileBytes(`${vaultPath}/${conflictPath}`, decrypted);

            // Push local to S3
            const enc = await readAndEncrypt(
              action.path,
              vaultPath,
              encryptionKey,
              fileDataCache,
            );
            await s3Upload(`${s3Prefix}files/${action.path}.enc`, enc);
          }

          tombstones.delete(action.path);
          conflicts.push(action.path);
          break;
        }

        // ── Deleted locally, modified remotely → re-download ─────
        case "conflict-delete-local": {
          await downloadAndDecrypt(
            `${s3Prefix}files/${action.path}.enc`,
            `${vaultPath}/${action.path}`,
            encryptionKey,
            vaultPath,
            action.path,
          );
          const remoteEntry = remoteMap.get(action.path)!;
          await setMtime(`${vaultPath}/${action.path}`, remoteEntry.modified);
          mergedFiles.set(action.path, {
            path: remoteEntry.path,
            hash: remoteEntry.hash,
            modified: remoteEntry.modified,
          });
          tombstones.delete(action.path);
          conflicts.push(action.path);
          break;
        }

        // ── Modified locally, deleted remotely → re-upload ───────
        case "conflict-delete-remote": {
          const encrypted = await readAndEncrypt(
            action.path,
            vaultPath,
            encryptionKey,
            fileDataCache,
          );
          await s3Upload(`${s3Prefix}files/${action.path}.enc`, encrypted);
          tombstones.delete(action.path);
          conflicts.push(action.path);
          break;
        }
      }

      actionsDone++;
      editor.setSyncProgress({ total: actionsTotal, done: actionsDone });
    }

    checkAbort(signal);

    // 6. Upload merged manifest to S3 (live files + pruned tombstones)
    const mergedManifest: Manifest = {
      version: 2,
      files: [
        ...Array.from(mergedFiles.values()),
        ...pruneTombstones(tombstones),
      ],
    };
    const manifestJson = new TextEncoder().encode(
      JSON.stringify(mergedManifest),
    );
    const encManifest = await encryptBlob(manifestJson, encryptionKey);
    await s3Upload(`${s3Prefix}manifest.enc`, encManifest);

    // 7. Persist merged manifest as local base for next sync
    await saveBaseManifest(vaultPath, encryptionKey, mergedManifest);

    // 8. Refresh file tree if anything changed on disk
    const hadFsChanges = actions.some(
      (a) =>
        a.kind === "download" ||
        a.kind === "delete-local" ||
        a.kind === "conflict" ||
        a.kind === "conflict-delete-local",
    );
    if (hadFsChanges) {
      await files.refresh(vaultPath);
    }

    editor.setSyncStatus("synced");

    if (conflicts.length > 0) {
      toast.info(
        `Sync complete — ${conflicts.length} conflict(s). Conflict copies saved as .sync-conflict files.`,
      );
    }
  } catch (err) {
    if (signal.aborted) {
      editor.setSyncStatus("idle");
      return;
    }
    console.error("[s3sync] Sync failed:", err);
    editor.setSyncStatus("error");
    throw err;
  } finally {
    if (activeSyncAbort === abortController) {
      activeSyncAbort = null;
    }
  }
}

// ─── Sync credentials ────────────────────────────────────────────────────

let syncCredentials: {
  vaultPath: string;
  vaultId: string;
  encryptionKey: number[];
  s3Config: S3Config;
  options?: SyncOptions;
} | null = null;

/** Store sync credentials so the manual sync button works even without auto-sync. */
export function setSyncCredentials(
  vaultPath: string,
  vaultId: string,
  encryptionKey: number[],
  s3Config: S3Config,
  options?: SyncOptions,
): void {
  syncCredentials = { vaultPath, vaultId, encryptionKey, s3Config, options };
}

/** Clear stored sync credentials (e.g. on logout). */
export function clearSyncCredentials(): void {
  syncCredentials = null;
}

// ─── Auto-sync ───────────────────────────────────────────────────────────

let autoSyncInterval: ReturnType<typeof setInterval> | null = null;

/** Begin periodic background sync. Call once after unlock when S3 is configured. */
export function startAutoSync(
  vaultPath: string,
  vaultId: string,
  encryptionKey: number[],
  s3Config: S3Config,
  intervalMs: number = 5 * 60 * 1000,
  options?: SyncOptions,
): void {
  stopAutoSync();
  syncCredentials = { vaultPath, vaultId, encryptionKey, s3Config, options };

  // Run a sync immediately on start
  runQuietSync();

  autoSyncInterval = setInterval(() => {
    runQuietSync();
  }, intervalMs);
}

/** Stop periodic sync (does not clear credentials). */
export function stopAutoSync(): void {
  if (autoSyncInterval) {
    clearInterval(autoSyncInterval);
    autoSyncInterval = null;
  }
}

export async function runQuietSync(): Promise<void> {
  if (!syncCredentials || isSyncing()) return;
  try {
    const { vaultPath, vaultId, encryptionKey, s3Config, options } =
      syncCredentials;
    await syncToS3(vaultPath, vaultId, encryptionKey, s3Config, options);
  } catch {
    // Background sync failures are silent — status bar shows error state
  }
}
