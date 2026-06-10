import { commands } from "$lib/bindings";

export type { VaultProfile, VaultProfiles } from "$lib/bindings";
import type { VaultProfile, VaultProfiles } from "$lib/bindings";

export async function saveSession(
  mnemonic: string,
  vaultPath: string,
): Promise<void> {
  const r = await commands.saveSession(mnemonic, vaultPath);
  if (r.status === "error") throw r.error;
}

export async function loadSession(): Promise<VaultProfile | null> {
  const r = await commands.loadSession();
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function clearSession(): Promise<void> {
  const r = await commands.clearSession();
  if (r.status === "error") throw r.error;
}

export async function loadVaultProfiles(): Promise<VaultProfiles> {
  const r = await commands.loadVaultProfiles();
  if (r.status === "error") throw r.error;
  return r.data;
}

export async function saveVaultProfile(profile: VaultProfile): Promise<void> {
  const r = await commands.saveVaultProfile(profile);
  if (r.status === "error") throw r.error;
}

export async function deleteVaultProfile(vaultPath: string): Promise<void> {
  const r = await commands.deleteVaultProfile(vaultPath);
  if (r.status === "error") throw r.error;
}
