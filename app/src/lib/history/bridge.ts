import { invoke } from "@tauri-apps/api/core";

export interface Snapshot {
  filename: string;
  /** Unix timestamp (seconds) */
  timestamp: number;
  /** Size in bytes */
  size: number;
}

export async function saveSnapshot(
  vaultPath: string,
  filePath: string,
  content: Uint8Array,
): Promise<string> {
  return invoke<string>("save_snapshot", {
    vaultPath,
    filePath,
    content: Array.from(content),
  });
}

export async function listSnapshots(
  vaultPath: string,
  filePath: string,
): Promise<Snapshot[]> {
  return invoke<Snapshot[]>("list_snapshots", { vaultPath, filePath });
}

export async function readSnapshot(
  vaultPath: string,
  filePath: string,
  snapshotFilename: string,
): Promise<Uint8Array> {
  const result = await invoke<number[]>("read_snapshot", {
    vaultPath,
    filePath,
    snapshotFilename,
  });
  return new Uint8Array(result);
}

export async function deleteSnapshot(
  vaultPath: string,
  filePath: string,
  snapshotFilename: string,
): Promise<void> {
  return invoke("delete_snapshot", { vaultPath, filePath, snapshotFilename });
}

export async function clearSnapshots(
  vaultPath: string,
  filePath: string,
): Promise<number> {
  return invoke<number>("clear_snapshots", { vaultPath, filePath });
}

export async function clearHistoryTree(
  vaultPath: string,
  entryPath: string,
): Promise<void> {
  return invoke("clear_history_tree", { vaultPath, entryPath });
}

export async function renameHistory(
  vaultPath: string,
  oldPath: string,
  newPath: string,
): Promise<void> {
  return invoke("rename_history", { vaultPath, oldPath, newPath });
}
