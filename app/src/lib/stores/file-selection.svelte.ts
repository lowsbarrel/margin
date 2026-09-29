import type { TreeEntry } from '$lib/fs/bridge';
import { remapPath } from '$lib/utils/path-remap';

export interface SelectedEntry {
	path: string;
	isDir: boolean;
}

// A plain Map, not SvelteMap: one dependency for every row's isSelected.
let entries = $state.raw<Map<string, SelectedEntry>>(new Map());
let lastPath = $state<string | null>(null);

// Rebuilt in one shot: $state.raw compares identity, and a Map mutated in place never notifies.
function selectionMap(
	source: Iterable<readonly [string, SelectedEntry]>
): Map<string, SelectedEntry> {
	return new Map(source);
}

export const fileSelection = {
	get entries(): Map<string, SelectedEntry> {
		return entries;
	},
	get lastPath(): string | null {
		return lastPath;
	},
	set lastPath(path: string | null) {
		lastPath = path;
	},

	replace(next: Iterable<readonly [string, SelectedEntry]>): void {
		entries = selectionMap(next);
	},

	followOpened(path: string): void {
		if (entries.has(path)) return;
		this.replace([[path, { path, isDir: false }]]);
		lastPath = path;
	},

	selectSingle(path: string, isDir: boolean): void {
		this.replace([[path, { path, isDir }]]);
		lastPath = path;
	},

	selectToggle(path: string, isDir: boolean): void {
		const current = [...entries];
		this.replace(
			entries.has(path)
				? current.filter(([p]) => p !== path)
				: [...current, [path, { path, isDir }]]
		);
		lastPath = path;
	},

	selectRange(path: string, rows: TreeEntry[]): void {
		if (!lastPath) {
			const row = rows.find((r) => r.path === path);
			if (row) {
				this.replace([[path, { path, isDir: row.is_dir }]]);
				lastPath = path;
			}
			return;
		}
		const anchorIdx = rows.findIndex((r) => r.path === lastPath);
		const targetIdx = rows.findIndex((r) => r.path === path);
		if (anchorIdx < 0 || targetIdx < 0) return;
		const lo = Math.min(anchorIdx, targetIdx);
		const hi = Math.max(anchorIdx, targetIdx);
		this.replace(
			rows
				.slice(lo, hi + 1)
				.map((r): [string, SelectedEntry] => [r.path, { path: r.path, isDir: r.is_dir }])
		);
	},

	selectAll(rows: TreeEntry[]): void {
		this.replace(
			rows.map((r): [string, SelectedEntry] => [r.path, { path: r.path, isDir: r.is_dir }])
		);
	},

	prune(live: Set<string>, hiddenByCollapsedAncestor: (path: string) => boolean): void {
		if (entries.size > 0) {
			const kept = [...entries].filter(
				([path]) => live.has(path) || hiddenByCollapsedAncestor(path)
			);
			if (kept.length !== entries.size) this.replace(kept);
		}
		if (lastPath && !live.has(lastPath) && !hiddenByCollapsedAncestor(lastPath)) lastPath = null;
	},

	remap(from: string, to: string, isDir: boolean): void {
		if (entries.size > 0) {
			this.replace(
				[...entries].map(([path, entry]) => {
					const next = remapPath(path, from, to, isDir);
					return [next, { path: next, isDir: entry.isDir }] as const;
				})
			);
		}
		if (lastPath) lastPath = remapPath(lastPath, from, to, isDir);
	}
};
