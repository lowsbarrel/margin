import type { Editor } from '@tiptap/core';
import {
	handleTauriFileDrop,
	insertFileAtCursor,
	setCursorAtCoords
} from '$lib/editor/handlers/drag-drop';
import type { AttachmentTarget } from '$lib/editor/attachments';
import { drag } from '$lib/stores/drag.svelte';
import {
	clearExternalEditorHandlers,
	setExternalEditorHandlers,
	type CssPoint
} from '$lib/utils/external-drop';

export function acceptPendingInsert(
	container: HTMLElement | undefined,
	target: AttachmentTarget | null
): void {
	const pending = drag.pendingInsert;
	if (!pending || !container || !target) return;
	const rect = container.getBoundingClientRect();
	const inside =
		pending.x >= rect.left &&
		pending.x <= rect.right &&
		pending.y >= rect.top &&
		pending.y <= rect.bottom;
	if (!inside) return;
	drag.clearPendingInsert();
	setCursorAtCoords(target.editor, pending.x, pending.y);
	void insertFileAtCursor(pending.path, target);
}

export function registerEditorDropTarget(
	rich: () => Editor | null,
	target: () => AttachmentTarget | null
): () => void {
	const handlers = {
		over: (pos: CssPoint) => {
			const editor = rich();
			if (editor) setCursorAtCoords(editor, pos.x, pos.y);
		},
		drop: (paths: string[], pos: CssPoint) => {
			const attachment = target();
			if (attachment) void handleTauriFileDrop(paths, pos, attachment);
		}
	};
	setExternalEditorHandlers(handlers);
	return () => clearExternalEditorHandlers(handlers);
}
