import { loadTabContent, tabFromContent } from '$lib/stores/tab-content';
import { getTabType, nextPaneId, nextTabId } from '$lib/stores/pane-tabs';
import type { Pane, Tab } from '$lib/stores/panes.svelte';
import type { WorkspacePane } from '$lib/settings/workspace';

export interface RestoredLayout {
	panes: Pane[];
	flexes: number[];
	activePaneIndex: number;
}

export async function restoreLayout(
	wsPanes: WorkspacePane[],
	wsFlexes: number[],
	wsActivePaneIndex: number
): Promise<RestoredLayout | null> {
	const buildTab = async (wsTab: WorkspacePane['tabs'][number]): Promise<Tab | null> => {
		const path = wsTab.path;
		const type = getTabType(path);
		try {
			const loaded = await loadTabContent(path, type);
			return tabFromContent(nextTabId(), path, type, loaded, {
				viewMode: wsTab.view_mode === 'source' ? 'source' : 'rich',
				pinned: wsTab.pinned ?? false,
				cursorPos: wsTab.cursor_pos ?? undefined
			});
		} catch {
			return null;
		}
	};

	const panes: Pane[] = [];
	for (const wsPane of wsPanes) {
		const built = await Promise.all(wsPane.tabs.map((t) => buildTab(t)));
		const tabs = built.filter((t): t is Tab => t !== null);
		if (tabs.length === 0) continue;
		const activeTabIndex = Math.min(Math.max(wsPane.active_tab_index, 0), tabs.length - 1);
		panes.push({
			id: nextPaneId(),
			tabs,
			activeTabIndex,
			externalContentVersion: 0
		});
	}

	if (panes.length === 0) return null;

	return {
		panes,
		flexes: wsFlexes.length === panes.length ? wsFlexes : panes.map(() => 1),
		activePaneIndex: Math.min(wsActivePaneIndex, panes.length - 1)
	};
}
