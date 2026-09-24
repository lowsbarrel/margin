import { panes } from '$lib/stores/panes.svelte';
import { remapPath } from '$lib/utils/path-remap';
import { terminals } from '$lib/stores/terminals.svelte';
import { files } from '$lib/stores/files.svelte';
import { editor } from '$lib/stores/editor.svelte';
import { noteFocus } from '$lib/stores/note-focus';
import { vault } from '$lib/stores/vault.svelte';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';
import {
	deleteEntry,
	fileExists,
	unwatchFile,
	unwatchVault,
	renameEntry,
	searchFiles,
	writeFileBytes
} from '$lib/fs/bridge';
import { createUniqueFilePath } from '$lib/utils/sidebar-ops';
import { renameHistory } from '$lib/history/bridge';
import { flushEditorWrites } from '$lib/fs/write-queue';
import { stopAutoSync, clearSyncCredentials } from '$lib/sync/s3sync';

export async function handleRename(oldPath: string, newPath: string, isDir = false) {
	if (!vault.vaultPath || oldPath === newPath) return;
	try {
		// A queued save for the old path would recreate the file there once the rename has landed.
		await flushEditorWrites();
		await unwatchFile();

		let historyRenamed = false;
		try {
			await renameHistory(vault.vaultPath, oldPath, newPath);
			historyRenamed = true;
		} catch (err) {
			console.warn('Failed to rename history:', err);
		}

		try {
			await renameEntry(oldPath, newPath);
		} catch (err) {
			if (historyRenamed) {
				renameHistory(vault.vaultPath, newPath, oldPath).catch((revertErr) =>
					console.warn('Failed to revert history rename:', revertErr)
				);
			}
			throw err;
		}

		// The rename has already landed on disk: reconcile the stores, and never re-throw from here.
		try {
			panes.remapPaths(oldPath, newPath, isDir);
			files.remapPaths(oldPath, newPath, isDir);

			if (files.activeFile) {
				files.setActiveFile(remapPath(files.activeFile, oldPath, newPath, isDir));
			}
			if (files.selectedFolder) {
				files.setSelectedFolder(remapPath(files.selectedFolder, oldPath, newPath, isDir));
			}
			await files.refresh(vault.vaultPath);
			await panes.restoreWatchingForPane(panes.activePaneIndex);
		} catch (postErr) {
			console.error('Rename succeeded but post-rename reconciliation failed:', postErr);
		}
	} catch (err) {
		console.error('Rename failed:', err);
		throw err;
	}
}

function forgetEntry(path: string, isDir: boolean) {
	panes.removePaths(path, isDir);
	if (
		files.activeFile &&
		(files.activeFile === path || (isDir && files.activeFile.startsWith(`${path}/`)))
	) {
		files.setActiveFile(null);
	}
	if (
		files.selectedFolder &&
		(files.selectedFolder === path || (isDir && files.selectedFolder.startsWith(`${path}/`)))
	) {
		files.setSelectedFolder(vault.vaultPath);
	}
}

export async function handleDelete(path: string, isDir: boolean) {
	if (!vault.vaultPath) return;
	try {
		await unwatchFile();
		forgetEntry(path, isDir);
		await deleteEntry(path);
		await files.refresh(vault.vaultPath);
		await panes.restoreWatchingForPane(panes.activePaneIndex);
	} catch (err) {
		console.error('Delete failed:', err);
		throw err;
	}
}

export async function reconcileMovedOut(
	entries: { path: string; isDir: boolean }[]
): Promise<{ path: string; isDir: boolean }[]> {
	if (!vault.vaultPath) return [];
	const remaining: { path: string; isDir: boolean }[] = [];
	const gone: { path: string; isDir: boolean }[] = [];
	for (const entry of entries) {
		((await fileExists(entry.path)) ? remaining : gone).push(entry);
	}
	if (gone.length === 0) return remaining;
	await unwatchFile();
	for (const entry of gone) forgetEntry(entry.path, entry.isDir);
	await files.refresh(vault.vaultPath);
	await panes.restoreWatchingForPane(panes.activePaneIndex);
	return remaining;
}

export function newEntryFolder(): string | null {
	const selected = files.selectedEntry;
	if (selected?.isDir) return selected.path;
	const active = files.activeFile;
	if (active) return active.slice(0, active.lastIndexOf('/'));
	return vault.vaultPath;
}

export async function ensureFolderExpanded(path: string) {
	const vaultPath = vault.vaultPath;
	if (!vaultPath || path === vaultPath || files.expandedFolders.has(path)) return;
	await files.expandFolder(path);
}

export async function handleNewNote(folder?: string, desiredName?: string) {
	const base = folder ?? newEntryFolder();
	const vaultPath = vault.vaultPath;
	if (!base || !vaultPath) return;

	const path = await createUniqueFilePath(base, desiredName);
	if (!path) return;

	await writeFileBytes(path, new TextEncoder().encode(''));
	await ensureFolderExpanded(base);
	await files.refresh(vaultPath);
	editor.markLocalChange();
	files.requestTreeReveal(path);
	if (path.endsWith('.md')) noteFocus.request(path);
	await panes.openFile(path);
}

export async function handleNewFolder(folder?: string) {
	const base = folder ?? newEntryFolder();
	if (!base || !vault.vaultPath) return;
	await ensureFolderExpanded(base);
	files.startNewFolder(base);
	files.requestPendingNewFolderReveal(base);
}

export async function handleWikiLink(title: string) {
	if (!vault.vaultPath) return;
	const results = await searchFiles(vault.vaultPath, title);
	const match = results.find(
		(r) => !r.is_dir && (r.name === `${title}.md` || r.name === `${title}.canvas`)
	);
	if (match) {
		await panes.openFile(match.path);
	} else {
		toast.info(m.toast_note_not_found({ title }));
	}
}

export function handleLogout(onBeforeLogout?: () => void) {
	onBeforeLogout?.();
	stopAutoSync();
	clearSyncCredentials();
	terminals.reset();
	panes.reset();
	files.clear();
	unwatchFile();
	unwatchVault();
	editor.setSyncStatus('idle');
	editor.setDirty(false);
	vault.lock();
}
