import { deriveVaultKeys } from '$lib/crypto/bridge';
import { fileExists, readFileBytes, setVaultDirectory, writeFileBytes } from '$lib/fs/bridge';
import type { VaultProfile } from '$lib/session/bridge';
import { unlockVaultProfile } from '$lib/session/bridge';
import { vault } from '$lib/stores/vault.svelte';
import * as m from '$lib/paraglide/messages.js';

export async function loadVaultProfiles(): Promise<{
	profiles: VaultProfile[];
	lastUsed: VaultProfile | null;
}> {
	const data = await vault.getVaultProfiles();
	const lastUsed = data.profiles.find((profile) => profile.vault_path === data.last_used) ?? null;
	return { profiles: data.profiles, lastUsed };
}

export async function unlockVault(
	mnemonic: string,
	vaultPath: string,
	name: string
): Promise<void> {
	const { vault_id } = await deriveVaultKeys(mnemonic);
	// The vault boundary must be set before any vault-scoped fs call, or a stale path makes them fail.
	await setVaultDirectory(vaultPath);
	await verifyOrInitVaultId(vaultPath, vault_id);
	vault.unlock(vault_id, vaultPath, name, mnemonic);
}

// Unlocks from the stored mnemonic in Rust: the mnemonic never reaches this code.
export async function unlockSavedVault(vaultPath: string, name: string): Promise<void> {
	const { vault_id } = await unlockVaultProfile(vaultPath);
	await setVaultDirectory(vaultPath);
	await verifyOrInitVaultId(vaultPath, vault_id);
	vault.unlock(vault_id, vaultPath, name);
}

async function verifyOrInitVaultId(vaultPath: string, vaultId: string): Promise<void> {
	const idFile = `${vaultPath}/.margin/vault.id`;
	if (await fileExists(idFile)) {
		const stored = new TextDecoder().decode(await readFileBytes(idFile)).trim();
		if (stored !== vaultId) throw new Error(m.login_error_wrong_passphrase());
		return;
	}
	await writeFileBytes(idFile, new TextEncoder().encode(vaultId));
}
