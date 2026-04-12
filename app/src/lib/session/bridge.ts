import { invoke } from "@tauri-apps/api/core";

export interface SavedSession {
  mnemonic: string;
  vault_path: string;
}

export interface VaultProfile {
  name: string;
  mnemonic: string;
  vault_path: string;
}

export interface VaultProfiles {
  profiles: VaultProfile[];
  last_used: string | null;
}

export async function saveSession(
  mnemonic: string,
  vaultPath: string,
): Promise<void> {
  return invoke("save_session", { mnemonic, vaultPath });
}

export async function loadSession(): Promise<VaultProfile | null> {
  return invoke<VaultProfile | null>("load_session");
}

export async function clearSession(): Promise<void> {
  return invoke("clear_session");
}

export async function loadVaultProfiles(): Promise<VaultProfiles> {
  return invoke<VaultProfiles>("load_vault_profiles");
}

export async function saveVaultProfile(profile: VaultProfile): Promise<void> {
  return invoke("save_vault_profile", { profile });
}

export async function deleteVaultProfile(vaultPath: string): Promise<void> {
  return invoke("delete_vault_profile", { vaultPath });
}
