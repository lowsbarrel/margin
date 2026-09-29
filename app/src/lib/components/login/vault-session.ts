import { deriveVaultKeys } from '$lib/crypto/bridge';
import {
	createDirectory,
	fileExists,
	readFileBytes,
	setVaultDirectory,
	writeFileBytes
} from '$lib/fs/bridge';
import type { VaultProfile } from '$lib/session/bridge';
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
	const keys = await deriveVaultKeys(mnemonic);
	await verifyOrInitVaultId(vaultPath, keys.vault_id);
	await setVaultDirectory(vaultPath);
	vault.unlock(keys, vaultPath, mnemonic, name);
}

async function verifyOrInitVaultId(vaultPath: string, vaultId: string): Promise<void> {
	const idFile = `${vaultPath}/.margin/vault.id`;
	if (await fileExists(idFile)) {
		const stored = new TextDecoder().decode(await readFileBytes(idFile)).trim();
		if (stored !== vaultId) throw new Error(m.login_error_wrong_passphrase());
		return;
	}
	await createDirectory(`${vaultPath}/.margin`);
	await writeFileBytes(idFile, new TextEncoder().encode(vaultId));
}
