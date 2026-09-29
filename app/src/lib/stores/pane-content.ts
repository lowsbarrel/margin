import { remapPath } from '$lib/utils/path-remap';
import type { Pane, Tab } from '$lib/stores/panes.svelte';

export function revokeBlobUrls(tabs: Tab[]): void {
	for (const t of tabs) {
		if (t.blobUrl) URL.revokeObjectURL(t.blobUrl);
	}
}

export function remapPanePaths(panes: Pane[], from: string, to: string, isDir: boolean): void {
	for (let pi = 0; pi < panes.length; pi++) {
		panes[pi].tabs = panes[pi].tabs.map((tab) => ({
			...tab,
			path: remapPath(tab.path, from, to, isDir)
		}));
	}
}

export function removePathsFromPanes(panes: Pane[], affected: (path: string) => boolean): void {
	for (let pi = 0; pi < panes.length; pi++) {
		const pane = panes[pi];
		const removed = pane.tabs.filter((tab) => affected(tab.path));
		if (removed.length === 0) continue;
		revokeBlobUrls(removed);

		const active = pane.tabs[pane.activeTabIndex];
		const tabs = pane.tabs.filter((tab) => !affected(tab.path));
		panes[pi].tabs = tabs;
		if (tabs.length === 0) {
			panes[pi].activeTabIndex = -1;
		} else if (active && !affected(active.path)) {
			panes[pi].activeTabIndex = tabs.indexOf(active);
		} else {
			panes[pi].activeTabIndex = Math.min(Math.max(pane.activeTabIndex, 0), tabs.length - 1);
		}
	}
}

export function updatePanesShowing(
	panes: Pane[],
	path: string,
	content: string,
	options: { skipPaneIndex?: number; onlyWhenChanged?: boolean } = {}
): number {
	let updated = 0;
	for (let pi = 0; pi < panes.length; pi++) {
		if (pi === options.skipPaneIndex) continue;
		const pane = panes[pi];
		if (
			options.onlyWhenChanged &&
			!pane.tabs.some((t) => t.path === path && t.content !== content)
		) {
			continue;
		}
		if (!pane.tabs.some((t) => t.path === path)) continue;
		panes[pi] = {
			...pane,
			tabs: pane.tabs.map((t) => (t.path === path ? { ...t, content } : t)),
			externalContentVersion: pane.externalContentVersion + 1
		};
		updated++;
	}
	return updated;
}

export function applyContentToActiveTab(
	panes: Pane[],
	paneIndex: number,
	path: string,
	content: string
): boolean {
	const pane = panes[paneIndex];
	if (!pane) return false;
	const ti = pane.activeTabIndex;
	if (ti < 0 || ti >= pane.tabs.length) return false;
	if (pane.tabs[ti].path !== path) return false;
	if (pane.tabs[ti].content === content) return false;
	panes[paneIndex].tabs[ti] = { ...panes[paneIndex].tabs[ti], content };
	panes[paneIndex].externalContentVersion++;
	return true;
}
