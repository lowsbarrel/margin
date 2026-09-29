import { onVaultFsChanged, walkDirectory } from '$lib/fs/bridge';
import { baseName } from '$lib/utils/path';

const REFRESH_DEBOUNCE_MS = 150;

const paths = new Set<string>();
const byName = new Map<string, string>();
let vault: string | null = null;
let ready = false;
const subscribers = new Set<() => void>();
let unlisten: (() => void) | null = null;
let debounce: ReturnType<typeof setTimeout> | undefined;
let walking: Promise<void> | null = null;
let rerun = false;

function resetVaultIndex(vaultPath: string | null): void {
	vault = vaultPath;
	walking = null;
	rerun = false;
	paths.clear();
	byName.clear();
	ready = false;
}

export function vaultIndexReady(): boolean {
	return ready;
}

async function refreshVaultIndex(vaultPath: string): Promise<void> {
	try {
		const entries = await walkDirectory(vaultPath);
		if (vault !== vaultPath) return;
		paths.clear();
		byName.clear();
		for (const entry of entries) {
			if (entry.is_dir) continue;
			const rel = entry.path.slice(vaultPath.length + 1);
			paths.add(rel);
			const name = baseName(rel).toLowerCase();
			if (!byName.has(name)) byName.set(name, rel);
		}
		ready = true;
	} catch (err) {
		console.warn('Vault index refresh failed:', err);
	}
}

// Editors mount together, so a second request while a walk is in flight joins it instead of starting another.
function refreshIndex(): Promise<void> {
	if (!vault) return Promise.resolve();
	if (walking) return walking;
	const run = refreshVaultIndex(vault).then(() => {
		if (walking !== run) return;
		walking = null;
		for (const subscriber of subscribers) subscriber();
		if (!rerun) return;
		rerun = false;
		void refreshIndex();
	});
	walking = run;
	return run;
}

function scheduleRefresh(): void {
	clearTimeout(debounce);
	debounce = setTimeout(() => {
		debounce = undefined;
		if (walking) rerun = true;
		else void refreshIndex();
	}, REFRESH_DEBOUNCE_MS);
}

function listen(): void {
	if (unlisten || !vault) return;
	void onVaultFsChanged(scheduleRefresh).then((stop) => {
		if (vault && subscribers.size > 0 && !unlisten) unlisten = stop;
		else stop();
	});
}

function stopListening(): void {
	clearTimeout(debounce);
	debounce = undefined;
	unlisten?.();
	unlisten = null;
}

// Every mounted editor wants the same index, so the walk and its filesystem listener are shared.
export function watchVaultIndex(vaultPath: string | null, onRefresh: () => void): () => void {
	if (vault !== vaultPath) {
		stopListening();
		resetVaultIndex(vaultPath);
	}
	subscribers.add(onRefresh);
	listen();
	void refreshIndex();
	return () => {
		subscribers.delete(onRefresh);
		if (subscribers.size === 0) stopListening();
	};
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
