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
	over: (pos: CssPoint) => void;
	drop: (paths: string[], pos: CssPoint) => void;
}

let editorHandlers: ExternalEditorHandlers | null = null;

export function setExternalEditorHandlers(handlers: ExternalEditorHandlers) {
	editorHandlers = handlers;
}

export function clearExternalEditorHandlers(handlers: ExternalEditorHandlers) {
	if (editorHandlers === handlers) editorHandlers = null;
}

// Wry reports drag positions in CSS pixels on macOS and Linux, but device pixels on Windows.
export async function installExternalDropRouter(): Promise<() => void> {
	let scaleFactor = IS_WINDOWS
		? await getCurrentWindow()
				.scaleFactor()
				.catch(() => window.devicePixelRatio)
		: 1;

	let unlistenScale: (() => void) | null = null;
	if (IS_WINDOWS) {
		// The ratio changes when the window is dragged to a display with a different scale.
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
		if (zone?.kind === 'editor') editorHandlers?.over(pos);
	});

	return () => {
		unlistenScale?.();
		unlistenDrop();
	};
}

async function handleExternalDrop(paths: string[], pos: CssPoint) {
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
	await files.expandFolder(dir);
	editor.markLocalChange();
	toast.success(m.toast_imported_items({ count: String(imported) }));
}

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
