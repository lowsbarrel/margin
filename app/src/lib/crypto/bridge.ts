import { invoke } from "@tauri-apps/api/core";
import { commands } from "$lib/bindings";
import { fromBytes } from "$lib/ipc";

export type { VaultKeys } from "$lib/bindings";
import type { VaultKeys } from "$lib/bindings";

export async function generateMnemonic(): Promise<string> {
  const r = await commands.generateMnemonic();
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function deriveVaultKeys(mnemonic: string): Promise<VaultKeys> {
  const r = await commands.deriveVaultKeys(mnemonic);
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function encryptBlob(
  plaintext: Uint8Array,
  key: number[],
): Promise<Uint8Array> {
  const buffer = await invoke<ArrayBuffer>("encrypt_blob_cmd", plaintext, {
    headers: { "x-key": key.join(",") },
  });
  return fromBytes(buffer);
}

export async function decryptBlob(
  ciphertext: Uint8Array,
  key: number[],
): Promise<Uint8Array> {
  const buffer = await invoke<ArrayBuffer>("decrypt_blob_cmd", ciphertext, {
    headers: { "x-key": key.join(",") },
  });
  return fromBytes(buffer);
}
