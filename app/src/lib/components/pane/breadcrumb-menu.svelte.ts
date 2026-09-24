import { SvelteSet } from 'svelte/reactivity';
import type { FsEntry, TreeEntry } from '$lib/fs/bridge';
import { buildSubtree, walkDirectory } from '$lib/fs/bridge';
import { files } from '$lib/stores/files.svelte';
import { panes } from '$lib/stores/panes.svelte';

const MAX_MATCHES = 200;

function byFolderThenName(a: FsEntry, b: FsEntry): number {
	if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
	return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
}

export class BreadcrumbMenu {
	#listPath = '';
	#generation = 0;

	index = $state<number | null>(null);
	title = $state('');
	source = $state('');
	rows = $state<TreeEntry[]>([]);
	matches = $state<TreeEntry[]>([]);
	filter = $state('');
	focus = $state(0);
	keyboard = $state(false);
	expanded = new SvelteSet<string>();

	open = $derived(this.index !== null);
	filtering = $derived(this.filter !== '');
	visible = $derived(this.filtering ? this.matches : this.rows);
	focused = $derived(this.visible[this.focus] ?? null);

	async show(index: number, listPath: string, source: string): Promise<void> {
		this.index = index;
		this.source = source;
		this.filter = '';
		this.focus = 0;
		this.matches = [];
		this.expanded.clear();
		await this.list(listPath);
	}

	async list(path: string): Promise<void> {
		this.#listPath = path;
		this.title = path.slice(path.lastIndexOf('/') + 1);
		const rows = (await this.#fetch()) ?? [];
		this.rows = rows;
		const preferred = rows.findIndex((row) => row.path === this.source);
		this.focus = preferred >= 0 ? preferred : 0;
	}

	close(): void {
		this.#generation += 1;
		this.index = null;
		this.source = '';
		this.#listPath = '';
		this.expanded.clear();
		this.rows = [];
		this.matches = [];
		this.filter = '';
	}

	async moveFocus(delta: number): Promise<void> {
		const total = this.visible.length;
		if (total === 0) return;
		this.focus = Math.min(total - 1, Math.max(0, this.focus + delta));
	}

	async right(): Promise<void> {
		const row = this.focused;
		if (!row) return;
		if (this.filtering) {
			if (row.is_dir) await this.drill(row.path);
			return;
		}
		if (!row.is_dir) return;
		if (this.expanded.has(row.path)) {
			await this.moveFocus(1);
			return;
		}
		await this.#setExpanded(row, true);
	}

	async left(): Promise<void> {
		if (this.filtering) return;
		const row = this.focused;
		if (!row) return;
		if (row.is_dir && this.expanded.has(row.path)) {
			await this.#setExpanded(row, false);
			return;
		}
		const parent = this.parentIndex(this.focus);
		if (parent >= 0) this.focus = parent;
	}

	async activate(paneIndex: number): Promise<void> {
		const row = this.focused;
		if (!row) return;
		if (row.is_dir) {
			if (this.filtering) await this.drill(row.path);
			else await this.#setExpanded(row, !this.expanded.has(row.path));
			return;
		}
		await this.openFile(row.path, paneIndex);
	}

	drill(path: string): Promise<void> {
		return this.show(this.index ?? 0, path, this.source);
	}

	async openFile(path: string, paneIndex: number): Promise<void> {
		await panes.focusPane(paneIndex);
		this.close();
		await panes.openFile(path);
	}

	async type(char: string): Promise<void> {
		this.filter += char;
		this.focus = 0;
		await this.#fetchMatches();
	}

	async backspace(): Promise<void> {
		if (!this.filter) return;
		this.filter = this.filter.slice(0, -1);
		this.focus = 0;
		if (this.filtering) await this.#fetchMatches();
		else this.matches = [];
	}

	parentIndex(index: number): number {
		const depth = this.rows[index]?.depth ?? 0;
		for (let i = index - 1; i >= 0; i--) {
			if (this.rows[i].depth < depth) return i;
		}
		return -1;
	}

	async #setExpanded(row: TreeEntry, open: boolean): Promise<void> {
		if (open) this.expanded.add(row.path);
		else this.expanded.delete(row.path);
		const rows = await this.#fetch();
		if (!rows) return;
		this.rows = rows;
		const kept = rows.findIndex((r) => r.path === row.path);
		if (kept >= 0) this.focus = kept;
	}

	async #fetch(): Promise<TreeEntry[] | null> {
		if (!this.#listPath) return [];
		const generation = ++this.#generation;
		const rows = await buildSubtree(
			this.#listPath,
			0,
			[...this.expanded],
			files.sortOrder,
			files.hiddenPaths
		);
		return generation === this.#generation ? rows : null;
	}

	async #fetchMatches(): Promise<void> {
		if (!this.#listPath) return;
		const generation = ++this.#generation;
		const needle = this.filter.toLowerCase();
		const all = await walkDirectory(this.#listPath, false);
		if (generation !== this.#generation) return;
		this.matches = all
			.filter(
				(entry) =>
					!files.hiddenPaths.includes(entry.path) && entry.name.toLowerCase().includes(needle)
			)
			.sort(byFolderThenName)
			.slice(0, MAX_MATCHES)
			.map((entry) => ({ ...entry, depth: 0 }));
	}
}
