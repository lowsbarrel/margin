import { invoke } from "@tauri-apps/api/core";

export interface VaultKeys {
  vault_id: string;
  encryption_key: number[];
}

export async function generateMnemonic(): Promise<string> {
  return invoke<string>("generate_mnemonic");
}

export async function deriveVaultKeys(mnemonic: string): Promise<VaultKeys> {
  return invoke<VaultKeys>("derive_vault_keys", { mnemonic });
}

export async function encryptBlob(
  plaintext: Uint8Array,
  key: number[],
): Promise<Uint8Array> {
  const buffer = await invoke<ArrayBuffer>("encrypt_blob_cmd", plaintext, {
    headers: { "x-key": key.join(",") },
  });
  return new Uint8Array(buffer);
}

export async function decryptBlob(
  ciphertext: Uint8Array,
  key: number[],
): Promise<Uint8Array> {
  const buffer = await invoke<ArrayBuffer>("decrypt_blob_cmd", ciphertext, {
    headers: { "x-key": key.join(",") },
  });
  return new Uint8Array(buffer);
}
