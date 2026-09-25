import type { EditorView } from '@codemirror/view';
import { insertDroppedPaths } from '$lib/editor/live/attachments';
import { drag } from '$lib/stores/drag.svelte';
import {
	clearExternalEditorHandlers,
	setExternalEditorHandlers,
	type CssPoint
} from '$lib/utils/external-drop';

function placeCursor(view: EditorView, x: number, y: number): void {
	const pos = view.posAtCoords({ x, y });
	if (pos == null) return;
	view.dispatch({ selection: { anchor: pos } });
}

export function acceptPendingInsert(
	container: HTMLElement | undefined,
	view: EditorView | null
): void {
	const pending = drag.pendingInsert;
	if (!pending || !container || !view) return;
	const rect = container.getBoundingClientRect();
	const inside =
		pending.x >= rect.left &&
		pending.x <= rect.right &&
		pending.y >= rect.top &&
		pending.y <= rect.bottom;
	if (!inside) return;
	drag.clearPendingInsert();
	placeCursor(view, pending.x, pending.y);
	void insertDroppedPaths(view, [pending.path]);
}

// The OS-drop router already hit-tested this editor and converted Tauri's physical drop point to CSS pixels.
export function registerEditorDropTarget(view: () => EditorView | null): () => void {
	const handlers = {
		over: (pos: CssPoint) => {
			const current = view();
			if (current) placeCursor(current, pos.x, pos.y);
		},
		drop: (paths: string[], pos: CssPoint) => {
			const current = view();
			if (!current) return;
			placeCursor(current, pos.x, pos.y);
			void insertDroppedPaths(current, paths);
		}
	};
	setExternalEditorHandlers(handlers);
	return () => clearExternalEditorHandlers(handlers);
}
