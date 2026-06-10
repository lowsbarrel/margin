import { commands } from "$lib/bindings";
import { toBytes, fromBytes } from "$lib/ipc";

export type { Snapshot } from "$lib/bindings";
import type { Snapshot } from "$lib/bindings";

export async function saveSnapshot(
  vaultPath: string,
  filePath: string,
  content: Uint8Array,
): Promise<string> {
  const r = await commands.saveSnapshot(vaultPath, filePath, toBytes(content));
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function listSnapshots(
  vaultPath: string,
  filePath: string,
): Promise<Snapshot[]> {
  const r = await commands.listSnapshots(vaultPath, filePath);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function readSnapshot(
  vaultPath: string,
  filePath: string,
  snapshotFilename: string,
): Promise<Uint8Array> {
  const r = await commands.readSnapshot(vaultPath, filePath, snapshotFilename);
  if (r.status === "error") throw r.error;
  return fromBytes(r.data);
}

export async function deleteSnapshot(
  vaultPath: string,
  filePath: string,
  snapshotFilename: string,
): Promise<void> {
  const r = await commands.deleteSnapshot(
    vaultPath,
    filePath,
    snapshotFilename,
  );
  if (r.status === "error") throw r.error;
}

export async function clearSnapshots(
  vaultPath: string,
  filePath: string,
): Promise<number> {
  const r = await commands.clearSnapshots(vaultPath, filePath);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function clearHistoryTree(
  vaultPath: string,
  entryPath: string,
): Promise<void> {
  const r = await commands.clearHistoryTree(vaultPath, entryPath);
  if (r.status === "error") throw r.error;
}

export async function renameHistory(
  vaultPath: string,
  oldPath: string,
  newPath: string,
): Promise<void> {
  const r = await commands.renameHistory(vaultPath, oldPath, newPath);
  if (r.status === "error") throw r.error;
}
