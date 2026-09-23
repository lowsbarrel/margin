import { watchFile, unwatchFile } from '$lib/fs/bridge';
import { files } from '$lib/stores/files.svelte';
import { editor } from '$lib/stores/editor.svelte';
import { vault } from '$lib/stores/vault.svelte';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';
import { loadTabContent, tabFromContent } from '$lib/stores/tab-content';
import {
	applyContentToActiveTab,
	remapPanePaths,
	removePathsFromPanes,
	revokeBlobUrls,
	updatePanesShowing
} from '$lib/stores/pane-content';
import { activeTabOf, createEmptyPane, getTabType, nextTabId } from '$lib/stores/pane-tabs';
import { closedTabs } from '$lib/stores/closed-tabs.svelte';
import { closeOneTab, closeTabsExcept, closeUnpinnedTabs } from '$lib/stores/pane-close';
import {
	moveTabIntoPane,
	removeFlexAt,
	splitAtPane,
	splitTabToNewPane
} from '$lib/stores/pane-layout';
import { restoreLayout } from '$lib/stores/pane-restore';
import type { WorkspacePane } from '$lib/settings/workspace';

export type TabType = 'markdown' | 'image' | 'pdf' | 'canvas' | 'unknown';
export type ViewMode = 'rich' | 'source';

export interface Tab {
	id: number;
	path: string;
	content: string;
	type: TabType;
	viewMode: ViewMode;
	blobUrl?: string;
	pdfData?: Uint8Array;
	size?: number;
	modified?: number;
	pinned: boolean;
	cursorPos?: number;
}

export interface Pane {
	id: number;
	tabs: Tab[];
	activeTabIndex: number;
	externalContentVersion: number;
}

export function fileTitle(path: string): string {
	const name = path.split('/').pop() ?? '';
	if (name.endsWith('.md')) return name.slice(0, -3);
	if (name.endsWith('.canvas')) return name.slice(0, -7);
	return name;
}

export function toBreadcrumbs(path: string, vaultPath: string | null): string[] {
	if (!vaultPath) return [];
	const rel = path.slice(vaultPath.length + 1);
	const parts = rel.split('/');
	return parts.map((p, i) => (i === parts.length - 1 ? fileTitle(path) : p));
}

async function focusActiveTab(tab: Tab | null): Promise<void> {
	files.setActiveFile(tab?.path ?? null);
	editor.setDirty(false);
	if (tab?.type === 'markdown') await watchFile(tab.path);
}

let _panes = $state<Pane[]>([createEmptyPane()]);
let _paneFlexes = $state<number[]>([1]);
let _activePaneIndex = $state(0);
let _fileSelectGeneration = 0;

export const panes = {
	get list(): Pane[] {
		return _panes;
	},

	get flexes(): number[] {
		return _paneFlexes;
	},

	get activePaneIndex(): number {
		return _activePaneIndex;
	},

	get activePane(): Pane {
		return _panes[_activePaneIndex] ?? _panes[0];
	},

	get activeTab(): Tab | null {
		return activeTabOf(_panes[_activePaneIndex] ?? _panes[0]);
	},

	get canReopenClosedTab(): boolean {
		return closedTabs.count > 0;
	},

	set list(v: Pane[]) {
		_panes = v;
	},

	set flexes(v: number[]) {
		_paneFlexes = v;
	},

	set activePaneIndex(v: number) {
		_activePaneIndex = v;
	},

	async switchTab(paneIndex: number, tabIndex: number) {
		const pane = _panes[paneIndex];
		if (!pane || tabIndex === pane.activeTabIndex || tabIndex < 0 || tabIndex >= pane.tabs.length)
			return;

		if (paneIndex === _activePaneIndex) await unwatchFile();

		_panes[paneIndex].activeTabIndex = tabIndex;
		const tab = _panes[paneIndex].tabs[tabIndex];

		if (paneIndex !== _activePaneIndex) return;
		files.setActiveFile(tab.path);
		editor.setDirty(false);
		if (vault.vaultPath) files.revealFile(tab.path, vault.vaultPath);
		if (tab.type === 'markdown') await watchFile(tab.path);
	},

	async closeTab(paneIndex: number, tabIndex: number) {
		const result = closeOneTab(_panes, paneIndex, tabIndex);
		for (const t of result.closed) closedTabs.push(t);

		if (result.emptied) {
			if (_panes.length > 1) {
				await this.closePane(paneIndex);
				return;
			}
			if (paneIndex === _activePaneIndex) {
				files.setActiveFile(null);
				await unwatchFile();
			}
			return;
		}
		if (result.nextIndex !== null) await this.switchTab(paneIndex, result.nextIndex);
	},

	async closeOtherTabs(paneIndex: number, tabIndex: number) {
		const keepTab = _panes[paneIndex].tabs[tabIndex];
		const result = closeTabsExcept(_panes, paneIndex, tabIndex);
		for (const t of result.closed) closedTabs.push(t);
		if (paneIndex === _activePaneIndex) {
			await unwatchFile();
			await focusActiveTab(keepTab);
		}
	},

	async closeAllTabs(paneIndex: number) {
		const result = closeUnpinnedTabs(_panes, paneIndex);
		for (const t of result.closed) closedTabs.push(t);

		if (result.nextIndex !== null) {
			await this.switchTab(paneIndex, result.nextIndex);
			return;
		}
		if (_panes.length > 1) {
			await this.closePane(paneIndex);
			return;
		}
		if (paneIndex === _activePaneIndex) {
			files.setActiveFile(null);
			await unwatchFile();
		}
	},

	togglePin(paneIndex: number, tabIndex: number) {
		const pane = _panes[paneIndex];
		if (!pane) return;
		const tab = pane.tabs[tabIndex];
		if (!tab) return;
		const activeId = pane.activeTabIndex >= 0 ? (pane.tabs[pane.activeTabIndex]?.id ?? null) : null;
		const next = pane.tabs.map((t) => (t.id === tab.id ? { ...t, pinned: !t.pinned } : t));
		const pinned = next.filter((t) => t.pinned);
		const unpinned = next.filter((t) => !t.pinned);
		_panes[paneIndex].tabs = [...pinned, ...unpinned];
		const idx = activeId == null ? -1 : _panes[paneIndex].tabs.findIndex((t) => t.id === activeId);
		_panes[paneIndex].activeTabIndex =
			idx >= 0 ? idx : Math.min(_panes[paneIndex].activeTabIndex, next.length - 1);
	},

	async reopenClosedTab(): Promise<boolean> {
		const path = closedTabs.take();
		if (!path) return false;
		return this.openFile(path);
	},

	async focusPane(paneIndex: number) {
		if (paneIndex === _activePaneIndex) return;
		const target = _panes[paneIndex];
		if (!target) return;
		if (target.activeTabIndex >= target.tabs.length) {
			_panes[paneIndex].activeTabIndex = target.tabs.length - 1;
		}
		await unwatchFile();
		_activePaneIndex = paneIndex;
		await focusActiveTab(activeTabOf(_panes[paneIndex]));
	},

	async closePane(paneIndex: number) {
		if (_panes.length <= 1) return;
		revokeBlobUrls(_panes[paneIndex].tabs);
		if (paneIndex === _activePaneIndex) await unwatchFile();
		_paneFlexes = removeFlexAt(_paneFlexes, paneIndex);
		_panes = _panes.filter((_, i) => i !== paneIndex);
		const newActive = Math.min(_activePaneIndex, _panes.length - 1);
		_activePaneIndex = newActive;
		await focusActiveTab(activeTabOf(_panes[newActive]));
	},

	async restoreWatchingForPane(paneIndex: number) {
		const pane = _panes[paneIndex];
		if (!pane || pane.tabs.length === 0) {
			_panes[paneIndex].activeTabIndex = -1;
			if (paneIndex === _activePaneIndex) {
				files.setActiveFile(null);
				await unwatchFile();
			}
			return;
		}

		const currentActiveIndex = pane.activeTabIndex;
		const currentPath = activeTabOf(pane)?.path;

		if (currentPath) {
			if (paneIndex === _activePaneIndex) {
				files.setActiveFile(currentPath);
				await watchFile(currentPath);
			}
			return;
		}

		const fallbackIndex = Math.min(Math.max(currentActiveIndex, 0), pane.tabs.length - 1);
		_panes[paneIndex].activeTabIndex = -1;
		await this.switchTab(paneIndex, fallbackIndex);
	},

	async openFile(path: string): Promise<boolean> {
		if (!vault.vaultPath) return false;
		const gen = ++_fileSelectGeneration;
		const paneIndex = _activePaneIndex;
		const tabType = getTabType(path);

		const existingIndex = _panes[paneIndex].tabs.findIndex((t) => t.path === path);
		if (existingIndex >= 0) {
			await this.switchTab(paneIndex, existingIndex);
			return true;
		}

		files.revealFile(path, vault.vaultPath);
		files.setActiveFile(path);
		await unwatchFile();
		if (gen !== _fileSelectGeneration) return false;

		let loaded;
		try {
			loaded = await loadTabContent(path, tabType);
		} catch (err) {
			console.warn('Failed to read file:', path, err);
			toast.error(m.toast_file_read_failed({ error: String(err) }));
			return false;
		}
		if (gen !== _fileSelectGeneration) return false;

		const newTab = tabFromContent(nextTabId(), path, tabType, loaded);
		_panes[paneIndex].tabs = [..._panes[paneIndex].tabs, newTab];
		_panes[paneIndex].activeTabIndex = _panes[paneIndex].tabs.length - 1;

		await focusActiveTab(newTab);
		return true;
	},

	async openFileInNewPane(path: string, refPaneIndex: number, side: 'left' | 'right') {
		const { flexes, insertAt } = splitAtPane(_paneFlexes, refPaneIndex, side);
		_panes = [..._panes.slice(0, insertAt), createEmptyPane(), ..._panes.slice(insertAt)];
		_paneFlexes = flexes;
		_activePaneIndex = insertAt;
		await this.openFile(path);
	},

	async moveTabToPane(srcPaneIndex: number, srcTabIndex: number, destPaneIndex: number) {
		if (srcPaneIndex === destPaneIndex) return;
		const moved = moveTabIntoPane(_panes, _paneFlexes, srcPaneIndex, srcTabIndex, destPaneIndex);
		if (!moved) return;
		await unwatchFile();
		_panes = moved.panes;
		_paneFlexes = moved.flexes;
		_activePaneIndex = moved.activePaneIndex;
		const destTab = activeTabOf(_panes[_activePaneIndex]);
		if (destTab) await focusActiveTab(destTab);
	},

	async moveTabToNewPane(
		srcPaneIndex: number,
		srcTabIndex: number,
		refPaneIndex: number,
		side: 'left' | 'right'
	) {
		const moved = splitTabToNewPane(
			_panes,
			_paneFlexes,
			srcPaneIndex,
			srcTabIndex,
			refPaneIndex,
			side
		);
		if (!moved) return;
		await unwatchFile();
		_panes = moved.panes;
		_paneFlexes = moved.flexes;
		_activePaneIndex = moved.activePaneIndex;
		await focusActiveTab(moved.tab);
	},

	remapPaths(from: string, to: string, isDir: boolean) {
		remapPanePaths(_panes, from, to, isDir);
	},

	removePaths(path: string, isDir: boolean) {
		removePathsFromPanes(
			_panes,
			(tabPath) => tabPath === path || (isDir && tabPath.startsWith(`${path}/`))
		);
	},

	applyExternalContent(path: string, content: string): boolean {
		return applyContentToActiveTab(_panes, _activePaneIndex, path, content);
	},

	applyRestoredContent(path: string, content: string): number {
		return updatePanesShowing(_panes, path, content, { onlyWhenChanged: true });
	},

	broadcastContent(sourcePaneIndex: number, filePath: string, content: string) {
		if (_panes.length < 2) return;
		updatePanesShowing(_panes, filePath, content, { skipPaneIndex: sourcePaneIndex });
	},

	reset() {
		_panes.forEach((pane) => revokeBlobUrls(pane.tabs));
		_panes = [createEmptyPane()];
		_paneFlexes = [1];
		_activePaneIndex = 0;
		closedTabs.clear();
	},

	async restoreFromWorkspace(
		wsPanes: WorkspacePane[],
		wsFlexes: number[],
		wsActivePaneIndex: number
	) {
		const restored = await restoreLayout(wsPanes, wsFlexes, wsActivePaneIndex);
		if (!restored) return;
		_panes = restored.panes;
		_paneFlexes = restored.flexes;
		_activePaneIndex = restored.activePaneIndex;

		const at = activeTabOf(_panes[_activePaneIndex]);
		if (at) {
			files.setActiveFile(at.path);
			if (at.type === 'markdown') await watchFile(at.path);
		}
	}
};
