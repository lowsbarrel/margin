import { drag } from '$lib/stores/drag.svelte';
import { files } from '$lib/stores/files.svelte';
import { startPointerDrag } from '$lib/utils/drag-handler';
import { reconcileMovedOut } from '$lib/utils/page-actions';
import { startDrag as startNativeDrag } from '@crabnebula/tauri-plugin-drag';
import { onVaultFsChanged, type TreeEntry } from '$lib/fs/bridge';

/**
 * How long the pointer must stay outside the window before an in-app drag is
 * promoted to a native OS drag. A drag that merely brushes the edge of the
 * webview reports a point at or past the boundary for a frame or two; requiring
 * a sustained exit means only a deliberate one converts the gesture.
 */
const NATIVE_DRAG_EXIT_DWELL_MS = 250;
const NATIVE_DRAG_END_SETTLE_MS = 100;
/** How long to wait for the file manager to finish a move it reported as dropped. */
const MOVE_OUT_SETTLE_MS = 10_000;

/** When the pointer was first seen outside the window during the current drag. */
let outsideSince: number | null = null;

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

/**
 * Move the dragged entries into `folderPath` (the vault root included).
 *
 * Entries already in that folder, and folders dropped inside themselves or a
 * descendant, are filtered out rather than attempted: the backend would refuse
 * them, and the user sees no drag at all instead of a failed move.
 */
export async function moveEntriesInto(
	folderPath: string,
	onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>,
	setDropTarget: (path: string | null) => void
) {
	if (!drag.active || drag.item?.kind !== 'file') return;
	const entries = getDropEntries();
	const valid = entries.filter((entry) => {
		if (isDescendantOrSelf(entry.path, folderPath)) return false;
		return entry.path.slice(0, entry.path.lastIndexOf('/')) !== folderPath;
	});
	// End the pointer drag before the first await: the page-level mouseup handler
	// runs after this one and must not resolve the same drop a second time.
	drag.end();
	drag.setExternalDropTarget(null);
	setDropTarget(null);
	outsideSince = null;
	await applySequentially(valid, (path, isDir) => onmoveentry(path, folderPath, isDir));
}

/**
 * Start a native OS drag once the pointer has deliberately left the window.
 *
 * The drag offers a move, so dropping on the Finder/Explorer moves the entry
 * out of the vault. The file manager performs the move; the app deletes
 * nothing itself, so a target that only reads the file leaves it in place.
 * Afterwards the app closes whatever actually left the vault.
 *
 * "Deliberately left" is the pointer strictly outside the viewport bounds
 * (`clientX < 0 || clientX >= innerWidth || clientY < 0 || clientY >=
 * innerHeight`) for {@link NATIVE_DRAG_EXIT_DWELL_MS}. Neither the boundary
 * point nor a one-frame excursion counts. If the webview stops delivering
 * mousemove as soon as the pointer leaves, the native drag is never started,
 * and the gesture stays in-app.
 */
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

	// Land any debounced save now: once the file manager has moved the note, a
	// late write would recreate it at its old path.
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

/**
 * Reconcile a drop outside the window. A file manager may report the drop
 * before it finishes moving, so anything still on disk is checked again on the
 * next filesystem change.
 */
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
