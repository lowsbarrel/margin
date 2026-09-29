import { drag } from '$lib/stores/drag.svelte';
import { files } from '$lib/stores/files.svelte';
import { startPointerDrag } from '$lib/utils/drag-handler';
import { reconcileMovedOut } from '$lib/utils/page-actions';
import { parentDir } from '$lib/utils/path';
import { startDrag as startNativeDrag } from '@crabnebula/tauri-plugin-drag';
import { onVaultFsChanged, type TreeEntry } from '$lib/fs/bridge';

const NATIVE_DRAG_EXIT_DWELL_MS = 250;
const NATIVE_DRAG_END_SETTLE_MS = 100;
const MOVE_OUT_SETTLE_MS = 10_000;

let outsideSince: number | null = null;

function isDescendantOrSelf(source: string, target: string): boolean {
	return target === source || target.startsWith(source + '/');
}

function getDropEntries(): { path: string; isDir: boolean }[] {
	const item = drag.item;
	if (!item || item.kind !== 'file') return [];
	if (files.selectedEntries.size > 1 && files.isSelected(item.path)) {
		return files.getSelectedAsList();
	}
	return [{ path: item.path, isDir: item.isDir }];
}

async function applySequentially(
	entries: { path: string; isDir: boolean }[],
	apply: (path: string, isDir: boolean) => Promise<void>
) {
	for (const entry of entries) {
		try {
			await apply(entry.path, entry.isDir);
		} catch (err) {
			console.warn(`Batch operation failed for ${entry.path}:`, err);
		}
	}
}

export async function moveEntriesInto(
	folderPath: string,
	onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>,
	setDropTarget: (path: string | null) => void
) {
	if (!drag.active || drag.item?.kind !== 'file') return;
	const entries = getDropEntries();
	const valid = entries.filter((entry) => {
		if (isDescendantOrSelf(entry.path, folderPath)) return false;
		return parentDir(entry.path) !== folderPath;
	});
	// End the drag before the first await: the page-level mouseup must not resolve it twice.
	drag.end();
	drag.setExternalDropTarget(null);
	setDropTarget(null);
	outsideSince = null;
	await applySequentially(valid, (path, isDir) => onmoveentry(path, folderPath, isDir));
}

export function tryNativeDrag(
	clientX: number,
	clientY: number,
	dragIconPath: string,
	nativeDragState: { started: boolean }
) {
	if (!drag.active || drag.nativeDragActive || nativeDragState.started) {
		outsideSince = null;
		return;
	}
	const item = drag.item;
	if (!item || item.kind !== 'file') {
		outsideSince = null;
		return;
	}

	const outside =
		clientX < 0 || clientY < 0 || clientX >= window.innerWidth || clientY >= window.innerHeight;
	if (!outside) {
		outsideSince = null;
		return;
	}
	const now = performance.now();
	if (outsideSince === null) {
		outsideSince = now;
		return;
	}
	if (now - outsideSince < NATIVE_DRAG_EXIT_DWELL_MS) return;
	outsideSince = null;

	nativeDragState.started = true;
	drag.end();
	drag.startNativeDrag();

	const entries: { path: string; isDir: boolean }[] =
		files.selectedEntries.size > 1 && files.isSelected(item.path)
			? files.getSelectedAsList()
			: [{ path: item.path, isDir: item.isDir }];

	let finished = false;
	function finishNativeDrag() {
		if (finished) return;
		finished = true;
		nativeDragState.started = false;
		window.setTimeout(() => {
			drag.endNativeDrag();
		}, NATIVE_DRAG_END_SETTLE_MS);
	}

	// Land the debounced save first: a write after the move recreates the note at its old path.
	window.dispatchEvent(new Event('margin:flush'));
	startNativeDrag(
		{ item: entries.map((entry) => entry.path), icon: dragIconPath, mode: 'move' },
		({ result }) => {
			finishNativeDrag();
			if (result === 'Dropped') void settleMovedOut(entries);
		}
	).catch(() => {
		finishNativeDrag();
	});
}

async function settleMovedOut(entries: { path: string; isDir: boolean }[]) {
	const remaining = await reconcileMovedOut(entries);
	if (remaining.length === 0) return;
	let settled = false;
	const unlisten = await onVaultFsChanged(() => {
		if (settled) return;
		settled = true;
		unlisten();
		void reconcileMovedOut(remaining);
	});
	window.setTimeout(() => {
		if (settled) return;
		settled = true;
		unlisten();
	}, MOVE_OUT_SETTLE_MS);
}

export function startDragEntry(e: MouseEvent, entry: TreeEntry, onDragStart: () => void) {
	const label = entry.name.replace(/\.(md|canvas)$/, '');
	startPointerDrag(e, { kind: 'file', path: entry.path, label, isDir: entry.is_dir }, onDragStart);
}
