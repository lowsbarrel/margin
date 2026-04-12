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
  const result = await invoke<number[]>("encrypt_blob", {
    plaintext: Array.from(plaintext),
    key,
  });
  return new Uint8Array(result);
}

export async function decryptBlob(
  ciphertext: Uint8Array,
  key: number[],
): Promise<Uint8Array> {
  const result = await invoke<number[]>("decrypt_blob", {
    ciphertext: Array.from(ciphertext),
    key,
  });
  return new Uint8Array(result);
}
