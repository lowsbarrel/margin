import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { IS_WINDOWS } from '$lib/utils/platform';
import { drag } from '$lib/stores/drag.svelte';
import { editor } from '$lib/stores/editor.svelte';
import { files } from '$lib/stores/files.svelte';
import { toast } from '$lib/stores/toast.svelte';
import { importExternalDirectory, importExternalFile, listDirectory } from '$lib/fs/bridge';
import { createUniquePath } from '$lib/utils/sidebar-ops';
import { dropDirectory, hitTestDropZone } from '$lib/utils/drop-zone';
import * as m from '$lib/paraglide/messages.js';

export interface CssPoint {
	x: number;
	y: number;
}

export interface ExternalEditorHandlers {
	/** Pointer moved over the editor while an OS file drag is in flight. */
	over: (pos: CssPoint) => void;
	/** Files released over the editor. */
	drop: (paths: string[], pos: CssPoint) => void;
}

/**
 * The active editor's insertion path. Exactly one editor is active at a time,
 * and it registers here so the router — which owns the single webview drag-drop
 * listener — can hand it the drops that land on it. Two listeners used to race
 * for the same event; one router with a registered sink cannot double-handle.
 */
let editorHandlers: ExternalEditorHandlers | null = null;

export function setExternalEditorHandlers(handlers: ExternalEditorHandlers) {
	editorHandlers = handlers;
}

export function clearExternalEditorHandlers(handlers: ExternalEditorHandlers) {
	if (editorHandlers === handlers) editorHandlers = null;
}

/**
 * Route OS drag-drop events to the tree, the editor, or nowhere.
 *
 * `DragDropEvent.position` is typed `PhysicalPosition`, but wry fills it from
 * the platform: on macOS (`draggingLocation`, points) and Linux (GTK widget
 * coordinates) it is already in CSS pixels; only Windows (`ScreenToClient`)
 * reports device pixels. Dividing the macOS value by the 2× retina factor put
 * the target half as far down as the cursor. The conversion happens once, here.
 */
export async function installExternalDropRouter(): Promise<() => void> {
	let scaleFactor = IS_WINDOWS
		? await getCurrentWindow()
				.scaleFactor()
				.catch(() => window.devicePixelRatio)
		: 1;

	// A window dragged to a display with a different pixel ratio keeps reporting
	// device pixels at the new ratio; without this every hit-test would drift
	// again until restart.
	let unlistenScale: (() => void) | null = null;
	if (IS_WINDOWS) {
		try {
			unlistenScale = await getCurrentWindow().onScaleChanged(({ payload }) => {
				scaleFactor = payload.scaleFactor;
			});
		} catch (err) {
			console.warn('Failed to watch the window scale factor:', err);
		}
	}

	const unlistenDrop = await getCurrentWebview().onDragDropEvent((event) => {
		const payload = event.payload;
		if (payload.type === 'leave') {
			drag.setExternalDropTarget(null);
			return;
		}
		const pos = {
			x: payload.position.x / scaleFactor,
			y: payload.position.y / scaleFactor
		};
		if (payload.type === 'drop') {
			drag.setExternalDropTarget(null);
			void handleExternalDrop(payload.paths, pos);
			return;
		}
		const zone = hitTestDropZone(pos.x, pos.y);
		drag.setExternalDropTarget(dropDirectory(zone));
		// Only over the editor: hovering a file across the sidebar used to move the
		// caret of whichever note was open.
		if (zone?.kind === 'editor') editorHandlers?.over(pos);
	});

	return () => {
		unlistenScale?.();
		unlistenDrop();
	};
}

async function handleExternalDrop(paths: string[], pos: CssPoint) {
	// A drop that is our own drag-out coming back is not an import — the file is
	// already in the vault and would be duplicated.
	if (drag.nativeDragActive) return;
	const zone = hitTestDropZone(pos.x, pos.y);
	const dir = dropDirectory(zone);
	if (dir) {
		await importPathsInto(paths, dir);
		return;
	}
	if (zone?.kind === 'editor') editorHandlers?.drop(paths, pos);
}

async function importPathsInto(paths: string[], dir: string) {
	let imported = 0;
	for (const source of paths) {
		const name = source.replace(/\\/g, '/').split('/').pop() ?? '';
		if (!name) continue;
		try {
			const dest = await createUniquePath(dir, name);
			if (await isDirectory(source)) await importExternalDirectory(source, dest);
			else await importExternalFile(source, dest);
			imported += 1;
		} catch (err) {
			toast.error(m.toast_import_file_failed({ name, error: String(err) }));
		}
	}
	if (imported === 0) return;
	// Expanding rebuilds the tree, so the imported entries appear under the folder
	// they landed in rather than behind a collapsed row. A collapsed target is
	// also the only way the user could fail to notice a successful import.
	await files.expandFolder(dir);
	editor.markLocalChange();
	toast.success(m.toast_imported_items({ count: String(imported) }));
}

/**
 * The drop payload carries paths only, so the type has to be read from the
 * parent listing — the bridge exposes no `stat`, and `listDirectory` is the one
 * call that works outside the vault.
 */
async function isDirectory(source: string): Promise<boolean> {
	const normalised = source.replace(/\\/g, '/');
	const slash = normalised.lastIndexOf('/');
	if (slash <= 0) return false;
	const name = normalised.slice(slash + 1);
	try {
		const entries = await listDirectory(normalised.slice(0, slash));
		return entries.some((entry) => entry.name === name && entry.is_dir);
	} catch {
		return false;
	}
}
