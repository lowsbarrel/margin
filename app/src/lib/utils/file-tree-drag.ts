import { drag } from '$lib/stores/drag.svelte';
import { files } from '$lib/stores/files.svelte';
import { startPointerDrag } from '$lib/utils/drag-handler';
import { startDrag as startNativeDrag } from '@crabnebula/tauri-plugin-drag';
import type { TreeEntry } from '$lib/fs/bridge';

const NATIVE_DRAG_END_SETTLE_MS = 100;

export function isDescendantOrSelf(source: string, target: string): boolean {
	return target === source || target.startsWith(source + '/');
}

export function getDropEntries(): { path: string; isDir: boolean }[] {
	const item = drag.item;
	if (!item || item.kind !== 'file') return [];
	if (files.selectedEntries.size > 1 && files.isSelected(item.path)) {
		return files.getSelectedAsList();
	}
	return [{ path: item.path, isDir: item.isDir }];
}

/**
 * Apply a per-entry filesystem operation to a multi-selection, one at a time.
 *
 * These used to be fired off in an un-awaited `for` loop, so moving three
 * notes launched three overlapping operations. Each one ends by refreshing the
 * file tree, and those refreshes interleaved — an early request could resolve
 * last and restore a snapshot still containing an entry that had already been
 * removed, so one of the three appeared to survive.
 *
 * Running them sequentially also means the destination-collision checks inside
 * the move handler see the results of the preceding moves rather than a stale
 * directory listing.
 */
async function applySequentially(
	entries: { path: string; isDir: boolean }[],
	apply: (path: string, isDir: boolean) => Promise<void>
) {
	for (const entry of entries) {
		try {
			await apply(entry.path, entry.isDir);
		} catch (err) {
			// One failure must not strand the rest of the batch.
			console.warn(`Batch operation failed for ${entry.path}:`, err);
		}
	}
}

export async function handleFolderDrop(
	e: MouseEvent,
	folderPath: string,
	onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>,
	setDropTarget: (path: string | null) => void
) {
	if (!drag.active || !drag.item || drag.item.kind !== 'file') return;
	e.stopPropagation();
	const entries = getDropEntries();
	const valid = entries.filter((entry) => {
		if (isDescendantOrSelf(entry.path, folderPath)) return false;
		const parent = entry.path.slice(0, entry.path.lastIndexOf('/'));
		return parent !== folderPath;
	});
	drag.end();
	setDropTarget(null);
	await applySequentially(valid, (path, isDir) => onmoveentry(path, folderPath, isDir));
}

export async function handleRootDrop(
	e: MouseEvent,
	vaultPath: string,
	onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>,
	setDropTarget: (path: string | null) => void
) {
	if (!drag.active || !drag.item || drag.item.kind !== 'file') return;
	const entries = getDropEntries();
	const valid = entries.filter((entry) => {
		const parent = entry.path.slice(0, entry.path.lastIndexOf('/'));
		return parent !== vaultPath;
	});
	drag.end();
	setDropTarget(null);
	await applySequentially(valid, (path, isDir) => onmoveentry(path, vaultPath, isDir));
}

/**
 * Start native OS drag when cursor exits the window.
 *
 * A drop outside the window is a copy: the OS hands the target application a
 * copy of the file. The vault entry is never removed — the OS reports `Dropped`
 * for both copy and move gestures, so treating a drop as a move deleted notes
 * the user had merely dragged somewhere.
 */
export function tryNativeDrag(
	clientX: number,
	clientY: number,
	dragIconPath: string,
	nativeDragState: { started: boolean }
) {
	if (!drag.active || drag.nativeDragActive || nativeDragState.started) return;
	const item = drag.item;
	if (!item || item.kind !== 'file') return;

	const margin = 2;
	const outside =
		clientX <= margin ||
		clientY <= margin ||
		clientX >= window.innerWidth - margin ||
		clientY >= window.innerHeight - margin;
	if (!outside) return;

	nativeDragState.started = true;
	drag.end();
	drag.startNativeDrag();

	const paths =
		files.selectedEntries.size > 1 && files.isSelected(item.path)
			? files.getSelectedPaths()
			: [item.path];

	let finished = false;
	function finishNativeDrag() {
		if (finished) return;
		finished = true;
		nativeDragState.started = false;
		window.setTimeout(() => {
			drag.endNativeDrag();
		}, NATIVE_DRAG_END_SETTLE_MS);
	}

	startNativeDrag({ item: paths, icon: dragIconPath }, () => {
		finishNativeDrag();
	}).catch(() => {
		finishNativeDrag();
	});
}

export function startDragEntry(e: MouseEvent, entry: TreeEntry, onDragStart: () => void) {
	const label = entry.name.replace(/\.(md|canvas)$/, '');
	startPointerDrag(e, { kind: 'file', path: entry.path, label, isDir: entry.is_dir }, onDragStart);
}
