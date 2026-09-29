import type { TreeEntry } from '$lib/fs/bridge';
import { files } from '$lib/stores/files.svelte';
import { parentDir } from '$lib/utils/path';
import type { TreeFocus } from './tree-focus.svelte';
import type { TreeWindow } from './tree-window.svelte';

export function createTreeKeyboard(options: {
	window: TreeWindow;
	focus: TreeFocus;
	onfileselect: (path: string) => void;
	ondeleterow: (entry: TreeEntry) => void;
}) {
	const { window: treeWindow, focus, onfileselect, ondeleterow } = options;

	function activate(row: TreeEntry) {
		files.selectSingle(row.path, row.is_dir);
		if (row.is_dir) {
			files.setSelectedFolder(row.path);
			void files.toggleFolder(row.path);
		} else {
			files.setSelectedFolder(null);
			onfileselect(row.path);
		}
	}

	async function focusPath(path: string) {
		focus.path = path;
		await treeWindow.focusRow(path);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.target instanceof HTMLInputElement) return;
		focus.keyboard = true;
		const path = focus.targetPath();
		if (!path) return;
		const rows = files.flatTree;
		const index = rows.findIndex((row) => row.path === path);
		if (index < 0) return;
		const row = rows[index];

		switch (e.key) {
			case 'ArrowDown':
				e.preventDefault();
				if (index + 1 < rows.length) void focusPath(rows[index + 1].path);
				break;
			case 'ArrowUp':
				e.preventDefault();
				if (index > 0) void focusPath(rows[index - 1].path);
				break;
			case 'ArrowRight':
				e.preventDefault();
				if (!row.is_dir) break;
				if (!files.expandedFolders.has(row.path)) void files.expandFolder(row.path);
				else if (rows[index + 1]?.depth > row.depth) void focusPath(rows[index + 1].path);
				break;
			case 'ArrowLeft': {
				e.preventDefault();
				if (row.is_dir && files.expandedFolders.has(row.path)) {
					void files.collapseFolder(row.path);
					break;
				}
				const parent = parentDir(row.path);
				if (rows.some((candidate) => candidate.path === parent)) void focusPath(parent);
				break;
			}
			case 'Enter':
				e.preventDefault();
				activate(row);
				break;
			case 'F2':
				e.preventDefault();
				files.startRename(row.path);
				break;
			case 'Delete':
			case 'Backspace':
				e.preventDefault();
				ondeleterow(row);
				break;
		}
	}

	return { handleKeydown };
}
