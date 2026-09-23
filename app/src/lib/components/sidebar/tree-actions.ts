import {
	copyDirectory,
	copyFile,
	fileExists,
	revealInFileManager,
	type FsEntry,
	type TreeEntry
} from '$lib/fs/bridge';
import { clipboard } from '$lib/stores/clipboard.svelte';
import { editor } from '$lib/stores/editor.svelte';
import { files } from '$lib/stores/files.svelte';
import { toast } from '$lib/stores/toast.svelte';
import { vault } from '$lib/stores/vault.svelte';
import { ensureFolderExpanded } from '$lib/utils/page-actions';
import { createUniquePath, normalizeDirName, normalizeFileName } from '$lib/utils/sidebar-ops';
import * as m from '$lib/paraglide/messages.js';

export interface TreeActionHost {
	renameEntry: (from: string, to: string, isDir: boolean) => Promise<void>;
	deleteEntry: (path: string, isDir: boolean) => Promise<void>;
}

export interface TreeActions {
	rename(entry: TreeEntry, newName: string): Promise<boolean>;
	deleteSelection(entry: FsEntry): Promise<void>;
	openInFinder(path: string): Promise<void>;
	duplicate(entry: FsEntry): Promise<void>;
	moveEntry(fromPath: string, toDir: string, isDir: boolean): Promise<void>;
	copy(entry?: FsEntry): void;
	cut(entry?: FsEntry): void;
	paste(targetDir: string): Promise<void>;
	pasteTarget(): string;
}

export function createTreeActions(host: TreeActionHost): TreeActions {
	async function rename(entry: TreeEntry, newName: string): Promise<boolean> {
		const sanitized = entry.is_dir ? normalizeDirName(newName) : normalizeFileName(newName);
		if (sanitized === null) return false;
		if (sanitized === entry.name) return true;

		const parent = entry.path.slice(0, entry.path.lastIndexOf('/'));
		const newPath = `${parent}/${sanitized}`;
		const differsBeyondCase = newPath.toLowerCase() !== entry.path.toLowerCase();
		if (differsBeyondCase && (await fileExists(newPath))) {
			toast.error(m.toast_path_exists({ name: sanitized }));
			return false;
		}

		try {
			await host.renameEntry(entry.path, newPath, entry.is_dir);
			return true;
		} catch (err) {
			toast.error(m.toast_rename_failed({ error: String(err) }));
			return false;
		}
	}

	async function deleteSelection(entry: FsEntry): Promise<void> {
		const selection = files.getSelectedAsList();
		const targets =
			selection.length > 1 && files.isSelected(entry.path)
				? selection
				: [{ path: entry.path, isDir: entry.is_dir }];

		const confirmed = window.confirm(
			targets.length > 1
				? m.sidebar_delete_many({ count: String(targets.length) })
				: `${m.sidebar_delete()} "${entry.name}"?`
		);
		if (!confirmed) return;

		// Sequential: each delete refreshes the tree, and overlapping refreshes can restore a stale snapshot.
		for (const target of targets) {
			try {
				await host.deleteEntry(target.path, target.isDir);
			} catch (err) {
				toast.error(m.toast_delete_failed({ error: String(err) }));
			}
		}
	}

	async function openInFinder(path: string): Promise<void> {
		try {
			await revealInFileManager(path);
		} catch (err) {
			toast.error(m.toast_open_finder_failed({ error: String(err) }));
		}
	}

	async function duplicate(entry: FsEntry): Promise<void> {
		if (!vault.vaultPath) return;
		const parent = entry.path.slice(0, entry.path.lastIndexOf('/'));
		const candidate = await createUniquePath(parent, entry.name, 'copy');

		try {
			if (entry.is_dir) await copyDirectory(entry.path, candidate);
			else await copyFile(entry.path, candidate);
			await ensureFolderExpanded(parent);
			await files.refresh(vault.vaultPath);
			selectAndReveal(candidate, entry.is_dir);
		} catch (err) {
			toast.error(m.toast_duplicate_failed({ error: String(err) }));
		}
	}

	function selectAndReveal(path: string, isDir: boolean) {
		files.setSelectedEntry(path, isDir);
		files.requestTreeReveal(path);
	}

	async function moveEntry(fromPath: string, toDir: string, isDir: boolean): Promise<void> {
		if (!vault.vaultPath) return;
		const name = fromPath.split('/').pop() ?? '';
		const dest = `${toDir}/${name}`;
		if (await fileExists(dest)) {
			toast.error(m.toast_exists_in_folder({ name }));
			return;
		}
		try {
			await host.renameEntry(fromPath, dest, isDir);
		} catch (err) {
			toast.error(m.toast_move_failed({ error: String(err) }));
		}
	}

	function copy(entry?: FsEntry) {
		if (entry && files.selectedEntries.size <= 1) {
			clipboard.copy([entry.path], [entry.is_dir]);
		} else {
			const sel = files.getSelectedAsList();
			if (sel.length === 0) return;
			clipboard.copy(
				sel.map((s) => s.path),
				sel.map((s) => s.isDir)
			);
		}
		toast.success(m.toast_copied());
	}

	function cut(entry?: FsEntry) {
		if (entry && files.selectedEntries.size <= 1) {
			clipboard.cut([entry.path], [entry.is_dir]);
		} else {
			const sel = files.getSelectedAsList();
			if (sel.length === 0) return;
			clipboard.cut(
				sel.map((s) => s.path),
				sel.map((s) => s.isDir)
			);
		}
		toast.success(m.toast_cut_clipboard());
	}

	async function paste(targetDir: string): Promise<void> {
		if (!vault.vaultPath || !clipboard.hasItems) return;
		const data = clipboard.consume();
		if (!data) return;

		for (let i = 0; i < data.paths.length; i++) {
			const srcPath = data.paths[i];
			const isDir = data.isDirs[i];
			const name = srcPath.split('/').pop() ?? '';
			const dest = await createUniquePath(targetDir, name);

			try {
				if (data.operation === 'copy') {
					if (isDir) await copyDirectory(srcPath, dest);
					else await copyFile(srcPath, dest);
				} else {
					await host.renameEntry(srcPath, dest, isDir);
				}
			} catch (err) {
				toast.error(m.toast_paste_failed({ error: String(err) }));
			}
		}

		if (targetDir !== vault.vaultPath) await files.expandFolder(targetDir);
		await files.refresh(vault.vaultPath);
		editor.markLocalChange();
	}

	function pasteTarget(): string {
		const sel = files.selectedEntry;
		if (sel?.isDir) return sel.path;
		if (files.selectedFolder) return files.selectedFolder;
		return vault.vaultPath ?? '';
	}

	return {
		rename,
		deleteSelection,
		openInFinder,
		duplicate,
		moveEntry,
		copy,
		cut,
		paste,
		pasteTarget
	};
}
