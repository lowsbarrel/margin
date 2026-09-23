import { panes, remapPath } from '$lib/stores/panes.svelte';
import { terminals } from '$lib/stores/terminals.svelte';
import { files } from '$lib/stores/files.svelte';
import { editor } from '$lib/stores/editor.svelte';
import { vault } from '$lib/stores/vault.svelte';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';
import {
	deleteEntry,
	unwatchFile,
	unwatchVault,
	renameEntry,
	searchFiles,
	writeFileBytes
} from '$lib/fs/bridge';
import { createUniqueFilePath } from '$lib/utils/sidebar-ops';
import { renameHistory } from '$lib/history/bridge';
import { flushEditorWrites } from '$lib/fs/writeQueue';
import { stopAutoSync, clearSyncCredentials } from '$lib/sync/s3sync';

export async function handleRename(oldPath: string, newPath: string, isDir = false) {
	if (!vault.vaultPath || oldPath === newPath) return;
	try {
		// The editor may still be holding a debounced save for the old path. Land
		// it before the move, or the queued write recreates the file at the old
		// path once the rename has already happened.
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

		// Ordering invariant: the filesystem rename (and its history counterpart)
		// has already succeeded above, so the entry IS renamed on disk. The
		// following steps only reconcile in-memory stores and watchers; a failure
		// in any of them must not abort the rest (which would leave stores stale)
		// and must not re-throw (which would surface a "rename failed" error to the
		// caller even though the rename itself succeeded).
		try {
			panes.remapPaths(oldPath, newPath, isDir);

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

export async function handleDelete(path: string, isDir: boolean) {
	if (!vault.vaultPath) return;
	try {
		await unwatchFile();

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

		await deleteEntry(path);
		await files.refresh(vault.vaultPath);
		await panes.restoreWatchingForPane(panes.activePaneIndex);
	} catch (err) {
		console.error('Delete failed:', err);
		throw err;
	}
}

/**
 * Folder a new note or folder lands in: the selected folder, else the folder of
 * the active file, else the vault root.
 */
export function newEntryFolder(): string | null {
	if (files.selectedFolder) return files.selectedFolder;
	const active = files.activeFile;
	if (active) return active.slice(0, active.lastIndexOf('/'));
	return vault.vaultPath;
}

async function ensureFolderExpanded(path: string) {
	const vaultPath = vault.vaultPath;
	if (!vaultPath || path === vaultPath || files.expandedFolders.has(path)) return;
	await files.expandFolder(path);
}

/** Create an empty note (or canvas, when `desiredName` names one) and open it. */
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
	await panes.openFile(path);
}

/** Start the inline "new folder" input inside `folder`. */
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
