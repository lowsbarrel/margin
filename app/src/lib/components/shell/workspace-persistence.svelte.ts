import { files } from '$lib/stores/files.svelte';
import { panes } from '$lib/stores/panes.svelte';
import { terminals } from '$lib/stores/terminals.svelte';
import { vault } from '$lib/stores/vault.svelte';
import {
	loadWorkspaceState,
	saveWorkspaceState,
	type WorkspaceState
} from '$lib/settings/workspace';

const SAVE_DEBOUNCE_MS = 1000;

type TimerHandle = ReturnType<typeof setTimeout>;

export const shellLayout = $state({ sidebarOpen: true, sidebarWidth: 280 });

let saveTimer: TimerHandle | null = null;
let restored = false;

function cancelScheduledSave(): void {
	if (saveTimer) clearTimeout(saveTimer);
	saveTimer = null;
}

export function saveWorkspace(): void {
	cancelScheduledSave();
	if (!vault.vaultPath || !vault.encryptionKey) return;
	const state: WorkspaceState = {
		panes: panes.list.map((p) => ({
			tabs: p.tabs.map((t) => ({
				path: t.path,
				type: t.type,
				pinned: t.pinned,
				view_mode: t.viewMode,
				cursor_pos: t.cursorPos ?? null
			})),
			active_tab_index: p.activeTabIndex
		})),
		pane_flexes: [...panes.flexes],
		active_pane_index: panes.activePaneIndex,
		expanded_folders: [...files.expandedFolders],
		sidebar_open: shellLayout.sidebarOpen,
		sidebar_width: shellLayout.sidebarWidth,
		sort_order: files.sortOrder,
		terminal_open: terminals.open,
		terminal_height: terminals.height
	};
	saveWorkspaceState(vault.vaultPath, vault.encryptionKey, state).catch((err) =>
		console.warn('Failed to save workspace state:', err)
	);
}

export function scheduleWorkspaceSave(): void {
	if (!vault.isUnlocked || !vault.vaultPath || !vault.encryptionKey) return;
	if (!restored) return;
	cancelScheduledSave();
	saveTimer = setTimeout(() => {
		saveTimer = null;
		saveWorkspace();
	}, SAVE_DEBOUNCE_MS);
}

export async function restoreWorkspace(): Promise<void> {
	if (!vault.vaultPath || !vault.encryptionKey) return;
	try {
		const ws = await loadWorkspaceState(vault.vaultPath, vault.encryptionKey);
		if (!ws || ws.panes.length === 0) {
			restored = true;
			return;
		}

		shellLayout.sidebarOpen = ws.sidebar_open;
		shellLayout.sidebarWidth = ws.sidebar_width ?? shellLayout.sidebarWidth;
		terminals.height = ws.terminal_height ?? terminals.height;
		if (ws.terminal_open) terminals.toggle();

		if (
			(ws.sort_order === 'name' || ws.sort_order === 'date') &&
			ws.sort_order !== files.sortOrder
		) {
			files.setSortOrder(ws.sort_order);
		}

		for (const folder of ws.expanded_folders) {
			files.expandedFolders.add(folder);
		}
		await files.refresh(vault.vaultPath);

		await panes.restoreFromWorkspace(
			ws.panes,
			ws.pane_flexes.map((f) => f ?? 1),
			ws.active_pane_index
		);
	} catch (err) {
		console.warn('Failed to restore workspace state:', err);
	}
	restored = true;
}

export function forgetWorkspaceRestore(): void {
	cancelScheduledSave();
	restored = false;
}
