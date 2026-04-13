import { invoke } from "@tauri-apps/api/core";
import type { ManifestEntry, Manifest } from "./s3sync-manifest";

// ─── Types matching Rust sync module ─────────────────────────────────────

export interface SyncAction {
  kind: string;
  path: string;
}

// ─── SHA-256 hashing ─────────────────────────────────────────────────────

/** Compute SHA-256 hashes for multiple files in a single native call (parallel). */
export async function hashFilesBatch(
  vaultPath: string,
  paths: string[],
): Promise<string[]> {
  return invoke<string[]>("hash_files_batch", { vaultPath, paths });
}

// ─── Manifest I/O ────────────────────────────────────────────────────────

/** Load and decrypt the local base manifest. Returns default if missing. */
export async function loadManifest(
  vaultPath: string,
  encryptionKey: number[],
): Promise<Manifest> {
  return invoke<Manifest>("load_manifest", { vaultPath, encryptionKey });
}

/** Encrypt and atomically save the base manifest to disk. */
export async function saveManifest(
  vaultPath: string,
  encryptionKey: number[],
  manifest: Manifest,
): Promise<void> {
  return invoke("save_manifest", { vaultPath, encryptionKey, manifest });
}

// ─── 3-way diff ──────────────────────────────────────────────────────────

/** Compute sync actions from base, local, and remote manifest entries. */
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

// ─── Tombstone helpers ───────────────────────────────────────────────────

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

// ─── Batch upload / download ─────────────────────────────────────────────

/** Read, encrypt, and upload files to S3 in a single native batch. */
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

/** Download, decrypt, and write files from S3 in a single native batch. */
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

/** Encrypt and upload manifest to S3. */
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
