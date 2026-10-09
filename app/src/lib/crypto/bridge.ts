import { commands } from '$lib/bindings';

export type { VaultId } from '$lib/bindings';
import type { VaultId } from '$lib/bindings';

export async function generateMnemonic(): Promise<string> {
	const r = await commands.generateMnemonic();
	if (r.status === 'error') throw r.error;
	return r.data;
}

export async function deriveVaultKeys(mnemonic: string): Promise<VaultId> {
	const r = await commands.deriveVaultKeys(mnemonic);
	if (r.status === 'error') throw r.error;
	return r.data;
}
