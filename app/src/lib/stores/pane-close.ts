import type { Pane, Tab } from '$lib/stores/panes.svelte';
import { revokeBlobUrls } from '$lib/stores/pane-content';

export interface CloseResult {
	closed: Tab[];
	nextIndex: number | null;
	emptied: boolean;
}

export function closeOneTab(panes: Pane[], paneIndex: number, tabIndex: number): CloseResult {
	const pane = panes[paneIndex];
	const closed = pane.tabs[tabIndex];
	revokeBlobUrls([closed]);

	const oldActiveIndex = pane.activeTabIndex;
	panes[paneIndex].tabs = pane.tabs.filter((_, i) => i !== tabIndex);

	if (panes[paneIndex].tabs.length === 0) {
		panes[paneIndex].activeTabIndex = -1;
		return { closed: [closed], nextIndex: null, emptied: true };
	}
	if (tabIndex === oldActiveIndex) {
		panes[paneIndex].activeTabIndex = -1;
		return {
			closed: [closed],
			nextIndex: Math.min(tabIndex, panes[paneIndex].tabs.length - 1),
			emptied: false
		};
	}
	if (tabIndex < oldActiveIndex) panes[paneIndex].activeTabIndex = oldActiveIndex - 1;
	return { closed: [closed], nextIndex: null, emptied: false };
}

export function closeTabsExcept(panes: Pane[], paneIndex: number, tabIndex: number): CloseResult {
	const pane = panes[paneIndex];
	const keepTab = pane.tabs[tabIndex];
	const closed = pane.tabs.filter((t, i) => i !== tabIndex && !t.pinned);
	const kept = pane.tabs.filter((t, i) => i === tabIndex || t.pinned);
	revokeBlobUrls(closed);
	panes[paneIndex].tabs = kept;
	panes[paneIndex].activeTabIndex = kept.findIndex((t) => t.id === keepTab.id);
	return { closed, nextIndex: null, emptied: false };
}

export function closeUnpinnedTabs(panes: Pane[], paneIndex: number): CloseResult {
	const pane = panes[paneIndex];
	const pinned = pane.tabs.filter((t) => t.pinned);
	const closed = pane.tabs.filter((t) => !t.pinned);
	revokeBlobUrls(closed);

	if (pinned.length === 0) {
		panes[paneIndex].tabs = [];
		panes[paneIndex].activeTabIndex = -1;
		return { closed, nextIndex: null, emptied: true };
	}
	panes[paneIndex].tabs = pinned;
	panes[paneIndex].activeTabIndex = -1;
	return { closed, nextIndex: 0, emptied: false };
}
