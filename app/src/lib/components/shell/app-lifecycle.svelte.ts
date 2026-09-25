import { onDestroy, onMount } from 'svelte';
import { getCurrentWindow, type CloseRequestedEvent } from '@tauri-apps/api/window';
import { editor } from '$lib/stores/editor.svelte';
import { files } from '$lib/stores/files.svelte';
import { panes } from '$lib/stores/panes.svelte';
import { terminals } from '$lib/stores/terminals.svelte';
import { vault } from '$lib/stores/vault.svelte';
import {
	onFileChanged,
	onVaultFsChanged,
	readFileBytes,
	rebuildIndex,
	unwatchFile
} from '$lib/fs/bridge';
import { saveSnapshot } from '$lib/history/bridge';
import { flushEditorWrites, isOwnRecentWrite } from '$lib/fs/write-queue';
import { stopAutoSync } from '$lib/sync/s3sync';
import { checkForAppUpdate } from '$lib/utils/updater';
import { saveWorkspace } from './workspace-persistence.svelte';

const UPDATE_INTERVAL_MS = 5 * 60 * 1000;
const VAULT_REFRESH_DEBOUNCE_MS = 300;
const CLOSE_FLUSH_TIMEOUT_MS = 3000;

type TimerHandle = ReturnType<typeof setTimeout>;
type IntervalHandle = ReturnType<typeof setInterval>;

export function initAppLifecycle(): void {
	let destroyed = false;
	let closing = false;
	let vaultRefreshTimer: TimerHandle | null = null;
	let updateInterval: IntervalHandle | null = null;
	const unlisteners: (() => void)[] = [];

	function attach(unlisten: () => void): void {
		if (destroyed) unlisten();
		else unlisteners.push(unlisten);
	}

	async function applyExternalChange(path: string): Promise<void> {
		const tab = panes.activeTab;
		if (!tab || tab.path !== path) return;
		try {
			const bytes = await readFileBytes(tab.path);
			if (isOwnRecentWrite(tab.path, bytes)) return;
			const newContent = new TextDecoder().decode(bytes);
			if (newContent === tab.content) return;
			if (vault.vaultPath) {
				try {
					await saveSnapshot(vault.vaultPath, tab.path, new TextEncoder().encode(tab.content));
				} catch (err) {
					console.warn('Failed to snapshot before applying external change:', err);
				}
			}
			if (panes.applyExternalContent(tab.path, newContent)) {
				editor.markLocalChange();
			}
		} catch (err) {
			console.warn('File may have been deleted:', err);
		}
	}

	function refreshAfterVaultChange(): void {
		editor.markLocalChange();
		if (vaultRefreshTimer) clearTimeout(vaultRefreshTimer);
		vaultRefreshTimer = setTimeout(() => {
			vaultRefreshTimer = null;
			files.refresh().catch((err) => console.warn('Failed to refresh file tree:', err));
			if (vault.vaultPath) void rebuildIndex(vault.vaultPath).catch(() => {});
		}, VAULT_REFRESH_DEBOUNCE_MS);
	}

	async function closeAfterFlushing(event: CloseRequestedEvent): Promise<void> {
		if (closing) return;
		closing = true;
		event.preventDefault();
		try {
			stopAutoSync();
			terminals.reset();
			await Promise.race([
				flushEditorWrites(),
				new Promise((resolve) => setTimeout(resolve, CLOSE_FLUSH_TIMEOUT_MS))
			]);
		} finally {
			try {
				await getCurrentWindow().destroy();
			} catch (err) {
				console.error('[close] window.destroy() failed:', err);
				closing = false;
			}
		}
	}

	onMount(() => {
		checkForAppUpdate();
		updateInterval = setInterval(checkForAppUpdate, UPDATE_INTERVAL_MS);

		onFileChanged(applyExternalChange).then(attach);
		onVaultFsChanged(refreshAfterVaultChange).then(attach);
		getCurrentWindow().onCloseRequested(closeAfterFlushing).then(attach);
	});

	onDestroy(() => {
		destroyed = true;
		if (vaultRefreshTimer) clearTimeout(vaultRefreshTimer);
		saveWorkspace();
		if (updateInterval) clearInterval(updateInterval);
		stopAutoSync();
		unwatchFile();
		for (const unlisten of unlisteners) unlisten();
	});
}
