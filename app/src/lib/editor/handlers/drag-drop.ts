import type { Editor } from '@tiptap/core';
import { TextSelection } from '@tiptap/pm/state';
import {
	captureInsertionPoint,
	insertDroppedPath,
	type AttachmentTarget
} from '$lib/editor/attachments';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';

export function setCursorAtCoords(editor: Editor, x: number, y: number): void {
	const result = editor.view.posAtCoords({ left: x, top: y });
	if (result == null) return;
	const pos = Math.min(result.pos, editor.view.state.doc.content.size);
	try {
		const tr = editor.view.state.tr.setSelection(TextSelection.create(editor.view.state.doc, pos));
		editor.view.dispatch(tr);
	} catch {}
}

export async function insertFileAtCursor(path: string, target: AttachmentTarget): Promise<void> {
	try {
		await insertDroppedPath(path, target, captureInsertionPoint(target.editor));
	} catch (err) {
		toast.error(m.toast_insert_file_failed({ error: String(err) }));
	}
}

// The OS-drop router already hit-tested this editor and converted Tauri's physical drop point to CSS pixels.
export async function handleTauriFileDrop(
	paths: string[],
	position: { x: number; y: number } | undefined,
	target: AttachmentTarget
): Promise<void> {
	if (position) setCursorAtCoords(target.editor, position.x, position.y);

	let cursor = captureInsertionPoint(target.editor);
	for (const path of paths) {
		try {
			cursor = await insertDroppedPath(path, target, cursor);
		} catch (err) {
			toast.error(m.toast_insert_file_failed({ error: String(err) }));
		}
	}
}
