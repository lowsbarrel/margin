import { clipboard } from '$lib/stores/clipboard.svelte';
import { files } from '$lib/stores/files.svelte';
import type { TreeActions } from './tree-actions';

export interface SidebarKeyboard {
	handleKeydown(event: KeyboardEvent): void;
}

export interface SidebarKeyboardHost {
	isPanelOpen: () => boolean;
	actions: TreeActions;
}

export function createSidebarKeyboard({
	isPanelOpen,
	actions
}: SidebarKeyboardHost): SidebarKeyboard {
	function handleKeydown(e: KeyboardEvent) {
		if (!isPanelOpen()) return;
		const el = document.activeElement;
		if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return;
		if (el instanceof HTMLElement && el.isContentEditable) return;

		const mod = e.metaKey || e.ctrlKey;
		if (!mod) return;
		const sel = files.selectedEntry;

		if (e.key === 'c' && sel) {
			e.preventDefault();
			e.stopPropagation();
			actions.copy();
		} else if (e.key === 'x' && sel) {
			e.preventDefault();
			e.stopPropagation();
			actions.cut();
		} else if (e.key === 'a') {
			e.preventDefault();
			e.stopPropagation();
			files.selectAll();
		} else if (e.key === 'v' && clipboard.hasItems) {
			e.preventDefault();
			e.stopPropagation();
			actions.paste(actions.pasteTarget());
		}
	}

	return { handleKeydown };
}
