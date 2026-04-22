import {
  s3Configure,
  s3Download,
  type S3Config,
} from "$lib/s3/bridge";
import { decryptBlob } from "$lib/crypto/bridge";
import {
  readFileBytes,
  writeFileBytes,
  walkDirectory,
  createDirectory,
  deleteEntry,
  setMtime,
} from "$lib/fs/bridge";
import { editor } from "$lib/stores/editor.svelte";
import { files } from "$lib/stores/files.svelte";
import { toast } from "$lib/stores/toast.svelte";
import type { ManifestEntry, Manifest } from "./s3sync-manifest";
import { validateManifest } from "./s3sync-manifest";
import { nowSeconds } from "./s3sync-diff";
import {
  hashFilesBatch,
  loadManifest,
  saveManifest,
  computeSyncActionsNative,
  collectTombstonesNative,
  mergeTombstonesNative,
  pruneTombstonesNative,
  syncUploadFiles,
  syncDownloadFiles,
  syncUploadManifest,
  syncDeleteFiles,
  pathToS3Key,
} from "./bridge";

// ─── Types ───────────────────────────────────────────────────────────────

export type ConflictStrategy = "local_wins" | "keep_newer";

export interface SyncOptions {
  conflictStrategy?: ConflictStrategy;
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

// sha256hex replaced by native hash_files_batch — see bridge.ts

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

// ─── Main sync ───────────────────────────────────────────────────────────

// readAndEncrypt replaced by native sync_upload_files — see bridge.ts

// downloadAndDecrypt replaced by native sync_download_files — see bridge.ts

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

    // 1. Load base manifest (last synced state) — fully in Rust
    checkAbort(signal);
    const baseManifest = await loadManifest(vaultPath, encryptionKey);
    const baseMap = new Map(baseManifest.files.map((e) => [e.path, e]));

    // 2. Build current local manifest
    //    Optimisation: skip hashing files whose mtime matches the base entry.
    //    Files that need hashing are batched into a single native call.
    const localFiles = await walkVault(vaultPath);
    checkAbort(signal);

    const localManifest: Manifest = { version: 3, files: [] };
    const unchangedEntries: ManifestEntry[] = [];
    const pathsToHash: string[] = [];
    const pathsMeta: { path: string; modified: number }[] = [];

    for (const file of localFiles) {
      const baseEntry = baseMap.get(file.path);
      if (
        baseEntry &&
        !baseEntry.deleted_at &&
        baseEntry.modified === file.modified
      ) {
        // mtime unchanged & not a tombstone — reuse previous hash
        unchangedEntries.push(baseEntry);
      } else {
        pathsToHash.push(file.path);
        pathsMeta.push({ path: file.path, modified: file.modified });
      }
    }

    // Batch hash all changed files in Rust (parallel SHA-256)
    const hashes =
      pathsToHash.length > 0
        ? await hashFilesBatch(vaultPath, pathsToHash)
        : [];
    checkAbort(signal);

    localManifest.files.push(...unchangedEntries);
    for (let i = 0; i < pathsToHash.length; i++) {
      localManifest.files.push({
        path: pathsMeta[i].path,
        hash: hashes[i],
        modified: pathsMeta[i].modified,
      });
    }

    // 3. Download remote manifest
    let remoteManifest: Manifest = { version: 3, files: [] };
    try {
      checkAbort(signal);
      const encManifest = await s3Download(`${s3Prefix}manifest.enc`);
      const decManifest = await decryptBlob(encManifest, encryptionKey);
      const parsed = validateManifest(
        JSON.parse(new TextDecoder().decode(decManifest)),
      );
      // Discard legacy v2 manifests — S3 keys used plaintext paths.
      // Treating it as empty forces a full re-upload with HMAC keys.
      if (parsed.version >= 3) {
        remoteManifest = parsed;
      }
    } catch (err) {
      if (signal.aborted) throw new Error("Sync cancelled");
      // If base has files but remote fetch fails, abort — otherwise the
      // 3-way diff treats every file as "deleted on remote" and wipes them.
      if (baseManifest.files.length > 0) {
        throw new Error(`Failed to download remote manifest: ${err}`);
      }
      // Base is empty — first sync, no remote manifest expected
    }

    // 4. Build maps & compute 3-way diff — fully in Rust
    const localMap = new Map(localManifest.files.map((e) => [e.path, e]));
    const remoteMap = new Map(remoteManifest.files.map((e) => [e.path, e]));
    const actions = await computeSyncActionsNative(
      baseManifest.files,
      localManifest.files,
      remoteManifest.files,
    );
    checkAbort(signal);

    // 5. Execute each action
    const conflicts: string[] = [];
    const mergedFiles = new Map<string, ManifestEntry>();
    let actionsDone = 0;
    const actionsTotal = actions.length;
    editor.setSyncProgress(
      actionsTotal > 0 ? { total: actionsTotal, done: 0 } : null,
    );

    // Collect tombstones from both sides and merge them — fully in Rust
    const baseTombstones = await collectTombstonesNative(baseManifest.files);
    const remoteTombstones = await collectTombstonesNative(remoteManifest.files);
    const mergedTombstonesList = await mergeTombstonesNative(
      baseTombstones,
      remoteTombstones,
    );
    const tombstones = new Map(mergedTombstonesList.map((e) => [e.path, e]));

    // Start with all local files in the merged result
    for (const entry of localManifest.files) mergedFiles.set(entry.path, entry);

    for (const action of actions) {
      checkAbort(signal);

      switch (action.kind) {
        // ── Local add / modify → push to S3 (native batch) ──────
        case "upload": {
          await syncUploadFiles(
            vaultPath,
            s3Prefix,
            [action.path],
            encryptionKey,
          );
          tombstones.delete(action.path);
          break;
        }

        // ── Remote add / modify → pull from S3 (native batch) ───
        case "download": {
          await syncDownloadFiles(
            vaultPath,
            s3Prefix,
            [action.path],
            encryptionKey,
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
            await syncDeleteFiles(s3Prefix, [action.path], encryptionKey);
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
            const localData = await readFileBytes(
              `${vaultPath}/${action.path}`,
            );
            const conflictPath = conflictCopyName(action.path);
            await ensureParentDir(vaultPath, conflictPath);
            await writeFileBytes(`${vaultPath}/${conflictPath}`, localData);

            await syncDownloadFiles(
              vaultPath,
              s3Prefix,
              [action.path],
              encryptionKey,
            );
            await setMtime(`${vaultPath}/${action.path}`, remoteEntry.modified);

            mergedFiles.set(action.path, {
              path: remoteEntry.path,
              hash: remoteEntry.hash,
              modified: remoteEntry.modified,
            });
          } else {
            // Local wins (default) → save remote as conflict copy
            const s3Key = await pathToS3Key(action.path, encryptionKey);
            const encrypted = await s3Download(
              `${s3Prefix}files/${s3Key}.enc`,
            );
            const decrypted = await decryptBlob(encrypted, encryptionKey);
            const conflictPath = conflictCopyName(action.path);
            await ensureParentDir(vaultPath, conflictPath);
            await writeFileBytes(`${vaultPath}/${conflictPath}`, decrypted);

            // Push local to S3
            await syncUploadFiles(
              vaultPath,
              s3Prefix,
              [action.path],
              encryptionKey,
            );
          }

          tombstones.delete(action.path);
          conflicts.push(action.path);
          break;
        }

        // ── Deleted locally, modified remotely → re-download ─────
        case "conflict-delete-local": {
          await syncDownloadFiles(
            vaultPath,
            s3Prefix,
            [action.path],
            encryptionKey,
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
          await syncUploadFiles(
            vaultPath,
            s3Prefix,
            [action.path],
            encryptionKey,
          );
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
    const prunedTombstones = await pruneTombstonesNative(
      Array.from(tombstones.values()),
      nowSeconds(),
    );
    const mergedManifest: Manifest = {
      version: 3,
      files: [...Array.from(mergedFiles.values()), ...prunedTombstones],
    };
    await syncUploadManifest(s3Prefix, encryptionKey, mergedManifest);

    // 7. Persist merged manifest as local base for next sync — fully in Rust
    await saveManifest(vaultPath, encryptionKey, mergedManifest);

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
