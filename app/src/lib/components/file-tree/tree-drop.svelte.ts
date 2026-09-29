import type { TreeEntry } from '$lib/fs/bridge';
import { drag } from '$lib/stores/drag.svelte';
import { vault } from '$lib/stores/vault.svelte';
import { hitTestDropZone } from '$lib/utils/drop-zone';
import { moveEntriesInto, startDragEntry, tryNativeDrag } from '$lib/utils/file-tree-drag';
import type { TreeFocus } from './tree-focus.svelte';

const REPEAT_CLICK_MS = 400;

export class TreeDrop {
	target = $state<string | null>(null);

	#focus: TreeFocus;
	#nativeDragState = { started: false };
	#suppressNextClick = false;
	#lastClickPath = '';
	#lastClickAt = 0;

	constructor(focus: TreeFocus) {
		this.#focus = focus;
	}

	isTarget(path: string): boolean {
		return this.target === path || drag.externalDropTarget === path;
	}

	trackPointer(x: number, y: number, iconPath: string) {
		tryNativeDrag(x, y, iconPath, this.#nativeDragState);
		const zone = hitTestDropZone(x, y);
		this.target = zone?.kind === 'folder' ? zone.path : null;
	}

	startPointerDrag(entry: TreeEntry, event: MouseEvent) {
		this.#focus.keyboard = false;
		startDragEntry(event, entry, () => {
			this.#suppressNextClick = true;
		});
	}

	takeSuppressedClick(): boolean {
		if (!this.#suppressNextClick) return false;
		this.#suppressNextClick = false;
		return true;
	}

	// A multi-click fires one click per press, so a folder must ignore the repeats of the triple-click that starts a rename.
	isRepeatClick(path: string): boolean {
		const now = performance.now();
		const repeated = path === this.#lastClickPath && now - this.#lastClickAt < REPEAT_CLICK_MS;
		this.#lastClickPath = path;
		this.#lastClickAt = now;
		return repeated;
	}

	dropFolderAt(x: number, y: number): string | null {
		const zone = hitTestDropZone(x, y);
		if (zone?.kind === 'folder' || zone?.kind === 'parent') return zone.path;
		return zone?.kind === 'tree-root' ? vault.vaultPath : null;
	}

	async settle(
		event: MouseEvent,
		onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>
	) {
		if (!drag.active || drag.item?.kind !== 'file') return;
		const folderPath = this.dropFolderAt(event.clientX, event.clientY);
		if (!folderPath) return;
		event.stopPropagation();
		await moveEntriesInto(folderPath, onmoveentry, (value) => {
			this.target = value;
		});
	}
}
