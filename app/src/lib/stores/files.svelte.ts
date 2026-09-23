import { SvelteSet } from 'svelte/reactivity';
import { buildVisibleTree, type TreeEntry } from '$lib/fs/bridge';
import { remapPath } from '$lib/utils/path-remap';
import { fileSelection, type SelectedEntry } from '$lib/stores/file-selection.svelte';

export type SortOrder = 'name' | 'date';
export type TreeRevealTarget =
	| { kind: 'entry'; path: string }
	| { kind: 'pending-new-folder'; parentPath: string };
export type { TreeEntry };
export type { SelectedEntry };

interface FilesState {
	flatTree: TreeEntry[];
	vaultRoot: string | null;
	activeFile: string | null;
	selectedFolder: string | null;
	loading: boolean;
	expandedFolders: SvelteSet<string>;
	pendingNewFolder: string | null;
	renamingPath: string | null;
	treeRevealTarget: TreeRevealTarget | null;
	treeRevealVersion: number;
	sortOrder: SortOrder;
}

const state = $state<FilesState>({
	flatTree: [],
	vaultRoot: null,
	activeFile: null,
	selectedFolder: null,
	loading: false,
	expandedFolders: new SvelteSet(),
	pendingNewFolder: null,
	renamingPath: null,
	treeRevealTarget: null,
	treeRevealVersion: 0,
	sortOrder: 'name'
});

let _rebuildGeneration = 0;

let hiddenPaths: string[] = [];

async function _rebuild(): Promise<void> {
	if (!state.vaultRoot) return;
	const generation = ++_rebuildGeneration;
	const expanded = [...state.expandedFolders];
	const flatTree = await buildVisibleTree(state.vaultRoot, expanded, state.sortOrder, hiddenPaths);
	// Rebuilds can resolve out of order; only the newest snapshot may land.
	if (generation !== _rebuildGeneration) return;
	state.flatTree = flatTree;
	_pruneSelection();
}

function hiddenByCollapsedAncestor(path: string, live: Set<string>): boolean {
	let slash = path.lastIndexOf('/');
	while (slash > 0) {
		const parent = path.slice(0, slash);
		if (live.has(parent) && !state.expandedFolders.has(parent)) return true;
		slash = parent.lastIndexOf('/');
	}
	return false;
}

function _pruneSelection(): void {
	const live = new Set(state.flatTree.map((r) => r.path));
	fileSelection.prune(live, (path) => hiddenByCollapsedAncestor(path, live));
}

function _requestTreeReveal(target: TreeRevealTarget) {
	state.treeRevealTarget = target;
	state.treeRevealVersion += 1;
}

export const files = {
	get flatTree() {
		return state.flatTree;
	},
	get activeFile() {
		return state.activeFile;
	},
	get selectedFolder() {
		return state.selectedFolder;
	},
	get loading() {
		return state.loading;
	},
	get expandedFolders() {
		return state.expandedFolders;
	},

	setActiveFile(path: string | null) {
		state.activeFile = path;
		if (path !== null) fileSelection.followOpened(path);
	},

	async revealFile(filePath: string, vaultPath: string) {
		const rel = filePath.slice(vaultPath.length + 1);
		const parts = rel.split('/');
		const before = state.expandedFolders.size;
		let current = vaultPath;
		for (let i = 0; i < parts.length - 1; i++) {
			current += '/' + parts[i];
			state.expandedFolders.add(current);
		}
		if (state.expandedFolders.size !== before) {
			await _rebuild();
		}
		_requestTreeReveal({ kind: 'entry', path: filePath });
	},

	requestTreeReveal(path: string) {
		_requestTreeReveal({ kind: 'entry', path });
	},

	requestPendingNewFolderReveal(parentPath: string) {
		_requestTreeReveal({ kind: 'pending-new-folder', parentPath });
	},

	get treeRevealTarget() {
		return state.treeRevealTarget;
	},

	get treeRevealVersion() {
		return state.treeRevealVersion;
	},

	setSelectedFolder(path: string | null) {
		state.selectedFolder = path;
	},

	remapPaths(oldPath: string, newPath: string, isDir: boolean) {
		const expanded = [...state.expandedFolders];
		state.expandedFolders.clear();
		for (const path of expanded) {
			state.expandedFolders.add(remapPath(path, oldPath, newPath, isDir));
		}
		fileSelection.remap(oldPath, newPath, isDir);
	},

	get selectedEntries() {
		return fileSelection.entries;
	},

	get selectedEntry(): SelectedEntry | null {
		const entries = fileSelection.entries;
		if (entries.size === 1) {
			return entries.values().next().value!;
		}
		return entries.size > 0
			? {
					path: fileSelection.lastPath!,
					isDir: entries.get(fileSelection.lastPath!)?.isDir ?? false
				}
			: null;
	},

	get lastSelectedPath() {
		return fileSelection.lastPath;
	},

	selectSingle(path: string, isDir: boolean) {
		fileSelection.selectSingle(path, isDir);
	},

	selectToggle(path: string, isDir: boolean) {
		fileSelection.selectToggle(path, isDir);
	},

	selectRange(path: string) {
		fileSelection.selectRange(path, state.flatTree);
	},

	isSelected(path: string) {
		return fileSelection.entries.has(path);
	},

	getSelectedPaths(): string[] {
		return [...fileSelection.entries.keys()];
	},

	getSelectedAsList(): SelectedEntry[] {
		return [...fileSelection.entries.values()];
	},

	clearSelection() {
		fileSelection.replace([]);
		fileSelection.lastPath = null;
	},

	selectAll() {
		fileSelection.selectAll(state.flatTree);
	},

	setSelectedEntry(path: string, isDir: boolean) {
		this.selectSingle(path, isDir);
	},

	async expandFolder(path: string) {
		state.expandedFolders.add(path);
		await _rebuild();
	},

	async collapseAll() {
		state.expandedFolders.clear();
		try {
			await _rebuild();
		} catch (err) {
			console.warn('Failed to rebuild tree after collapseAll:', err);
		}
	},

	async collapseFolder(path: string) {
		state.expandedFolders.delete(path);
		const idx = state.flatTree.findIndex((r) => r.path === path);
		if (idx !== -1) {
			const parentDepth = state.flatTree[idx].depth;
			let end = idx + 1;
			while (end < state.flatTree.length && state.flatTree[end].depth > parentDepth) {
				end++;
			}
			if (end > idx + 1) {
				const next = [...state.flatTree];
				next.splice(idx + 1, end - idx - 1);
				state.flatTree = next;
			}
		} else {
			await _rebuild();
		}
	},

	async toggleFolder(path: string) {
		if (state.expandedFolders.has(path)) {
			await this.collapseFolder(path);
		} else {
			await this.expandFolder(path);
		}
	},

	async refresh(vaultPath?: string) {
		if (vaultPath) state.vaultRoot = vaultPath;
		if (!state.vaultRoot) return;
		state.loading = true;
		try {
			await _rebuild();
		} finally {
			state.loading = false;
		}
	},

	setHiddenPaths(paths: string[]) {
		const next = [...paths].sort();
		if (next.length === hiddenPaths.length && next.every((p, i) => p === hiddenPaths[i])) return;
		hiddenPaths = next;
		_rebuild().catch((err) => console.warn('Failed to rebuild tree:', err));
	},

	clear() {
		state.flatTree = [];
		state.vaultRoot = null;
		state.activeFile = null;
		state.selectedFolder = null;
		fileSelection.replace([]);
		fileSelection.lastPath = null;
		state.expandedFolders.clear();
		state.pendingNewFolder = null;
		state.renamingPath = null;
		state.treeRevealTarget = null;
		state.treeRevealVersion = 0;
	},

	get pendingNewFolder() {
		return state.pendingNewFolder;
	},

	get sortOrder() {
		return state.sortOrder;
	},

	async setSortOrder(order: SortOrder) {
		state.sortOrder = order;
		await _rebuild();
	},

	async toggleSortOrder() {
		state.sortOrder = state.sortOrder === 'name' ? 'date' : 'name';
		await _rebuild();
	},

	startNewFolder(parentPath: string) {
		state.pendingNewFolder = parentPath;
	},

	cancelNewFolder() {
		state.pendingNewFolder = null;
	},

	get renamingPath() {
		return state.renamingPath;
	},

	startRename(path: string) {
		state.renamingPath = path;
	},

	cancelRename() {
		state.renamingPath = null;
	}
};
