import { commands } from "$lib/bindings";

export type { AppSettings } from "$lib/bindings";
import type { AppSettings } from "$lib/bindings";

export async function saveSettings(
  vaultPath: string,
  encryptionKey: number[],
  settings: AppSettings,
): Promise<void> {
  const r = await commands.saveSettings(vaultPath, encryptionKey, settings);
  if (r.status === "error") throw r.error;
}

export async function loadSettings(
  vaultPath: string,
  encryptionKey: number[],
): Promise<AppSettings | null> {
  const r = await commands.loadSettings(vaultPath, encryptionKey);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function exportSettingsString(
  encryptionKey: number[],
  settings: AppSettings,
): Promise<string> {
  const r = await commands.exportSettingsString(encryptionKey, settings);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function importSettingsString(
  encryptionKey: number[],
  encoded: string,
): Promise<AppSettings> {
  const r = await commands.importSettingsString(encryptionKey, encoded);
  if (r.status === "error") throw r.error;
  return r.data;
}
