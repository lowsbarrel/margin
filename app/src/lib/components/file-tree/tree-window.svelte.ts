import { tick } from 'svelte';
import type { TreeEntry } from '$lib/fs/bridge';
import { files, type TreeRevealTarget } from '$lib/stores/files.svelte';
import { vault } from '$lib/stores/vault.svelte';
import { ROW_HEIGHT } from './row-classes';

const OVERSCAN = 5;

export type VisibleItem =
	| { kind: 'row'; row: TreeEntry; virtualIndex: number }
	| { kind: 'new-folder'; virtualIndex: number };

export class TreeWindow {
	el = $state<HTMLDivElement | undefined>();
	clientHeight = $state(0);
	scrollTop = $state(0);
	revealHandledVersion = $state(0);

	#rows = $derived(files.flatTree);
	#insertIndex = $derived.by(() => {
		const parent = files.pendingNewFolder;
		if (!parent) return -1;
		const index = this.#rows.findIndex((row) => row.path === parent);
		if (index === -1) return 0;
		const depth = this.#rows[index].depth;
		let at = index + 1;
		while (at < this.#rows.length && this.#rows[at].depth > depth) at++;
		return at;
	});
	#count = $derived(this.#rows.length + (this.#insertIndex >= 0 ? 1 : 0));
	#start = $derived(Math.max(0, Math.floor(this.scrollTop / ROW_HEIGHT) - OVERSCAN));
	#end = $derived(
		Math.min(this.#count, Math.ceil((this.scrollTop + this.clientHeight) / ROW_HEIGHT) + OVERSCAN)
	);

	newFolderDepth = $derived.by(() => {
		const parent = files.pendingNewFolder;
		if (!parent || !vault.vaultPath) return 0;
		if (parent === vault.vaultPath) return 0;
		const index = this.#rows.findIndex((row) => row.path === parent);
		return index >= 0 ? this.#rows[index].depth + 1 : 0;
	});

	totalHeight = $derived(this.#count * ROW_HEIGHT);

	renderedItems = $derived.by((): VisibleItem[] =>
		[...this.#windowItems(), ...this.#pinnedItems()].sort((a, b) => a.virtualIndex - b.virtualIndex)
	);

	#windowItems(): VisibleItem[] {
		const items: VisibleItem[] = [];
		const insertIndex = this.#insertIndex;
		for (let virtualIndex = this.#start; virtualIndex < this.#end; virtualIndex++) {
			if (virtualIndex === insertIndex) {
				items.push({ kind: 'new-folder', virtualIndex });
				continue;
			}
			const rowIndex =
				insertIndex >= 0 && virtualIndex > insertIndex ? virtualIndex - 1 : virtualIndex;
			if (rowIndex >= 0 && rowIndex < this.#rows.length) {
				items.push({ kind: 'row', row: this.#rows[rowIndex], virtualIndex });
			}
		}
		return items;
	}

	// An edited row stays mounted even scrolled out of the window: detaching a focused input reports a blur that would commit a half-typed name.
	#pinnedItems(): VisibleItem[] {
		const items: VisibleItem[] = [];
		const insertIndex = this.#insertIndex;
		if (
			files.pendingNewFolder &&
			insertIndex >= 0 &&
			!(insertIndex >= this.#start && insertIndex < this.#end)
		) {
			items.push({ kind: 'new-folder', virtualIndex: insertIndex });
		}
		const renaming = files.renamingPath;
		if (renaming) {
			const row = this.#rows.find((candidate) => candidate.path === renaming);
			const index = row ? this.indexOf({ kind: 'entry', path: renaming }) : -1;
			if (row && index >= 0 && (index < this.#start || index >= this.#end)) {
				items.push({ kind: 'row', row, virtualIndex: index });
			}
		}
		return items;
	}

	indexOf(target: TreeRevealTarget): number {
		const insertIndex = this.#insertIndex;
		if (target.kind === 'pending-new-folder') {
			return files.pendingNewFolder === target.parentPath ? insertIndex : -1;
		}
		const rowIndex = this.#rows.findIndex((row) => row.path === target.path);
		if (rowIndex < 0) return -1;
		return insertIndex >= 0 && rowIndex >= insertIndex ? rowIndex + 1 : rowIndex;
	}

	scrollRowIntoView(index: number): boolean {
		const el = this.el;
		if (!el) return false;
		const visibleHeight = el.clientHeight || this.clientHeight;
		if (visibleHeight <= 0) return false;

		const rowTop = index * ROW_HEIGHT;
		const rowBottom = rowTop + ROW_HEIGHT;
		const currentTop = el.scrollTop;
		let nextTop: number;
		if (rowTop < currentTop) {
			nextTop = rowTop;
		} else if (rowBottom > currentTop + visibleHeight) {
			nextTop = rowBottom - visibleHeight;
		} else {
			return true;
		}

		const maxTop = Math.max(0, this.totalHeight - visibleHeight);
		const boundedTop = Math.max(0, Math.min(maxTop, nextTop));
		el.scrollTop = boundedTop;
		this.scrollTop = boundedTop;
		return true;
	}

	async focusRow(path: string) {
		const index = this.indexOf({ kind: 'entry', path });
		if (index >= 0) this.scrollRowIntoView(index);
		await tick();
		this.el?.querySelector<HTMLElement>(`[data-path="${CSS.escape(path)}"]`)?.focus();
	}

	reveal(target: TreeRevealTarget | null, version: number) {
		if (!target || !this.el || version === this.revealHandledVersion) return;
		const index = this.indexOf(target);
		if (index < 0) return;
		if (this.scrollRowIntoView(index)) this.revealHandledVersion = version;
	}
}
