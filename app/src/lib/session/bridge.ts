import { commands } from '$lib/bindings';

export type {
	VaultProfileInfo as VaultProfile,
	VaultProfilesInfo as VaultProfiles
} from '$lib/bindings';
import type { VaultId, VaultProfileInfo, VaultProfilesInfo } from '$lib/bindings';

export async function saveSession(mnemonic: string, vaultPath: string): Promise<void> {
	const r = await commands.saveSession(mnemonic, vaultPath);
	if (r.status === 'error') throw r.error;
}

export async function loadSession(): Promise<VaultProfileInfo | null> {
	const r = await commands.loadSession();
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function clearSession(): Promise<void> {
	const r = await commands.clearSession();
	if (r.status === 'error') throw r.error;
}

export async function loadVaultProfiles(): Promise<VaultProfilesInfo> {
	const r = await commands.loadVaultProfiles();
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function saveVaultProfile(
	name: string,
	vaultPath: string,
	mnemonic: string
): Promise<void> {
	const r = await commands.saveVaultProfile(name, vaultPath, mnemonic);
	if (r.status === 'error') throw r.error;
}

export async function renameVaultProfile(vaultPath: string, name: string): Promise<void> {
	const r = await commands.renameVaultProfile(vaultPath, name);
	if (r.status === 'error') throw r.error;
}

export async function deleteVaultProfile(vaultPath: string): Promise<void> {
	const r = await commands.deleteVaultProfile(vaultPath);
	if (r.status === 'error') throw r.error;
}

export async function unlockVaultProfile(vaultPath: string): Promise<VaultId> {
	const r = await commands.unlockVaultProfile(vaultPath);
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function revealMnemonic(vaultPath: string): Promise<string> {
	const r = await commands.revealMnemonic(vaultPath);
	if (r.status === 'error') throw r.error;
	return r.data;
}
