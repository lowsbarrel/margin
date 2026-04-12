import { invoke } from "@tauri-apps/api/core";
import type { S3Config } from "$lib/s3/bridge";

import type { ConflictStrategy } from "$lib/sync/s3sync";

export interface AppSettings {
  s3: S3Config | null;
  attachment_folder: string | null;
  auto_sync: boolean | null;
  conflict_strategy: ConflictStrategy | null;
}

export async function saveSettings(
  vaultPath: string,
  encryptionKey: number[],
  settings: AppSettings,
): Promise<void> {
  return invoke("save_settings", {
    vaultPath,
    encryptionKey,
    settings,
  });
}

export async function loadSettings(
  vaultPath: string,
  encryptionKey: number[],
): Promise<AppSettings | null> {
  return invoke<AppSettings | null>("load_settings", {
    vaultPath,
    encryptionKey,
  });
}

export async function exportSettingsString(
  encryptionKey: number[],
  settings: AppSettings,
): Promise<string> {
  return invoke<string>("export_settings_string", {
    encryptionKey,
    settings,
  });
}

export async function importSettingsString(
  encryptionKey: number[],
  encoded: string,
): Promise<AppSettings> {
  return invoke<AppSettings>("import_settings_string", {
    encryptionKey,
    encoded,
  });
}
