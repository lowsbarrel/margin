import { onVaultFsChanged, walkDirectory } from '$lib/fs/bridge';

const paths = new Set<string>();
const byName = new Map<string, string>();
let vault: string | null = null;

export function resetVaultIndex(vaultPath: string | null): void {
	if (vault === vaultPath) return;
	vault = vaultPath;
	paths.clear();
	byName.clear();
}

export async function refreshVaultIndex(vaultPath: string | null): Promise<void> {
	if (!vaultPath) return;
	if (vault !== vaultPath) resetVaultIndex(vaultPath);
	try {
		const entries = await walkDirectory(vaultPath);
		for (const entry of entries) {
			if (entry.is_dir) continue;
			const rel = entry.path.slice(vaultPath.length + 1);
			paths.add(rel);
			const name = rel.slice(rel.lastIndexOf('/') + 1).toLowerCase();
			if (!byName.has(name)) byName.set(name, rel);
		}
	} catch (err) {
		console.warn('Vault index refresh failed:', err);
	}
}

export function watchVaultIndex(vaultPath: string | null, onRefresh: () => void): () => void {
	void refreshVaultIndex(vaultPath).then(onRefresh);
	let unlisten: (() => void) | null = null;
	void onVaultFsChanged(() => {
		void refreshVaultIndex(vaultPath).then(onRefresh);
	}).then((stop) => {
		unlisten = stop;
	});
	return () => unlisten?.();
}

export function listVaultFiles(): string[] {
	return [...paths];
}

export function vaultFileExists(relPath: string): boolean {
	return paths.has(relPath);
}

export function findVaultFile(name: string): string | null {
	return byName.get(name.toLowerCase()) ?? null;
}
