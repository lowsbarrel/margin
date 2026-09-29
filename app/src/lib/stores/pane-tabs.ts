import { IMAGE_EXTS } from '$lib/utils/mime';
import type { Pane, Tab, TabType } from '$lib/stores/panes.svelte';

const PDF_EXTS: Record<string, true> = { '.pdf': true };

let _nextTabId = 0;
let _nextPaneId = 0;

export function nextTabId(): number {
	return _nextTabId++;
}

export function nextPaneId(): number {
	return _nextPaneId++;
}

export function getTabType(path: string): TabType {
	const ext = path.slice(path.lastIndexOf('.')).toLowerCase();
	if (ext === '.md') return 'markdown';
	if (ext === '.canvas') return 'canvas';
	if (IMAGE_EXTS.has(ext)) return 'image';
	if (PDF_EXTS[ext]) return 'pdf';
	return 'unknown';
}

export function createEmptyPane(): Pane {
	return {
		id: nextPaneId(),
		tabs: [],
		activeTabIndex: -1,
		externalContentVersion: 0
	};
}

export function activeTabOf(pane: Pane): Tab | null {
	const i = pane.activeTabIndex;
	return i >= 0 && i < pane.tabs.length ? pane.tabs[i] : null;
}
