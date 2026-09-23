import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import type { TreeEntry } from '$lib/fs/bridge';
import { vault } from '$lib/stores/vault.svelte';
import { handleNewFolder, handleNewNote } from '$lib/utils/page-actions';
import { buildMenuItems, type MenuTarget } from '$lib/utils/sidebar-menu';
import type { TreeActions } from './tree-actions';

export interface TreeMenu {
	readonly target: MenuTarget | null;
	readonly x: number;
	readonly y: number;
	readonly items: ContextMenuItem[];
	openRoot(event: MouseEvent): void;
	openEntry(entry: TreeEntry, event: MouseEvent): void;
	close(): void;
}

export function createTreeMenu(actions: TreeActions): TreeMenu {
	let target = $state<MenuTarget | null>(null);
	let x = $state(0);
	let y = $state(0);

	const items = $derived.by((): ContextMenuItem[] => {
		if (!target) return [];
		return buildMenuItems(target, {
			onNewFile: (base) => handleNewNote(base),
			onNewCanvas: (base) => handleNewNote(base, 'Untitled.canvas'),
			onNewFolder: (base) => handleNewFolder(base),
			onPaste: (dir) => actions.paste(dir),
			onOpenInFinder: (path) => actions.openInFinder(path),
			onCopy: (entry) => actions.copy(entry),
			onCut: (entry) => actions.cut(entry),
			onDuplicate: (entry) => actions.duplicate(entry),
			onDelete: (entry) => actions.deleteSelection(entry)
		});
	});

	function openRoot(event: MouseEvent) {
		if (!vault.vaultPath) return;
		if ((event.target as HTMLElement).closest('.tree-row')) return;
		event.preventDefault();
		// The freshly mounted menu listens on document for contextmenu; an event still bubbling would close it the same tick.
		event.stopPropagation();
		x = event.clientX;
		y = event.clientY;
		target = { kind: 'root', path: vault.vaultPath };
	}

	function openEntry(entry: TreeEntry, event: MouseEvent) {
		event.preventDefault();
		x = event.clientX;
		y = event.clientY;
		target = { kind: 'entry', entry };
	}

	function close() {
		target = null;
	}

	return {
		get target() {
			return target;
		},
		get x() {
			return x;
		},
		get y() {
			return y;
		},
		get items() {
			return items;
		},
		openRoot,
		openEntry,
		close
	};
}
