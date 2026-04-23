import { invoke } from "@tauri-apps/api/core";
import type { ManifestEntry, Manifest } from "./s3sync-manifest";

// ── Types ──

export interface SyncAction {
  kind: string;
  path: string;
}

// ── SHA-256 hashing ──

export async function hashFilesBatch(
  vaultPath: string,
  paths: string[],
): Promise<string[]> {
  return invoke<string[]>("hash_files_batch", { vaultPath, paths });
}

// ── Manifest I/O ──

/** Returns default manifest if missing. */
export async function loadManifest(
  vaultPath: string,
  encryptionKey: number[],
): Promise<Manifest> {
  return invoke<Manifest>("load_manifest", { vaultPath, encryptionKey });
}

export async function saveManifest(
  vaultPath: string,
  encryptionKey: number[],
  manifest: Manifest,
): Promise<void> {
  return invoke("save_manifest", { vaultPath, encryptionKey, manifest });
}

// ── 3-way diff ──

export async function computeSyncActionsNative(
  baseFiles: ManifestEntry[],
  localFiles: ManifestEntry[],
  remoteFiles: ManifestEntry[],
): Promise<SyncAction[]> {
  return invoke<SyncAction[]>("compute_sync_actions", {
    baseFiles,
    localFiles,
    remoteFiles,
  });
}

// ── Tombstone helpers ──

export async function collectTombstonesNative(
  files: ManifestEntry[],
): Promise<ManifestEntry[]> {
  return invoke<ManifestEntry[]>("collect_tombstones", { files });
}

export async function mergeTombstonesNative(
  a: ManifestEntry[],
  b: ManifestEntry[],
): Promise<ManifestEntry[]> {
  return invoke<ManifestEntry[]>("merge_tombstones", { a, b });
}

export async function pruneTombstonesNative(
  tombstones: ManifestEntry[],
  nowSeconds: number,
): Promise<ManifestEntry[]> {
  return invoke<ManifestEntry[]>("prune_tombstones", { tombstones, nowSeconds });
}

// ── Batch upload / download ──

export async function syncUploadFiles(
  vaultPath: string,
  s3Prefix: string,
  paths: string[],
  encryptionKey: number[],
): Promise<void> {
  return invoke("sync_upload_files", {
    vaultPath,
    s3Prefix,
    paths,
    encryptionKey,
  });
}

export async function syncDownloadFiles(
  vaultPath: string,
  s3Prefix: string,
  paths: string[],
  encryptionKey: number[],
): Promise<void> {
  return invoke("sync_download_files", {
    vaultPath,
    s3Prefix,
    paths,
    encryptionKey,
  });
}

export async function syncUploadManifest(
  s3Prefix: string,
  encryptionKey: number[],
  manifest: Manifest,
): Promise<void> {
  return invoke("sync_upload_manifest", {
    s3Prefix,
    encryptionKey,
    manifest,
  });
}

// ── Delete files from S3 ──

/** Computes HMAC keys internally. */
export async function syncDeleteFiles(
  s3Prefix: string,
  paths: string[],
  encryptionKey: number[],
): Promise<void> {
  return invoke("sync_delete_files", {
    s3Prefix,
    paths,
    encryptionKey,
  });
}

// ── Path → S3 key mapping ──

export async function pathToS3Key(
  relPath: string,
  encryptionKey: number[],
): Promise<string> {
  return invoke<string>("path_to_s3_key", { relPath, encryptionKey });
}
