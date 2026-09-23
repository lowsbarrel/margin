import type { Editor } from '@tiptap/core';
import { TextSelection } from '@tiptap/pm/state';
import {
	captureInsertionPoint,
	insertDroppedPath,
	type AttachmentTarget
} from '$lib/editor/attachments';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';

/** Move the editor cursor to a screen coordinate */
export function setCursorAtCoords(editor: Editor, x: number, y: number): void {
	const result = editor.view.posAtCoords({ left: x, top: y });
	if (result == null) return;
	const pos = Math.min(result.pos, editor.view.state.doc.content.size);
	try {
		const tr = editor.view.state.tr.setSelection(TextSelection.create(editor.view.state.doc, pos));
		editor.view.dispatch(tr);
	} catch {
		/* pos may be invalid for non-text nodes */
	}
}

/**
 * Insert a file dragged out of the tree into the active note.
 *
 * The caret was already moved to where the row was dropped, so the insertion
 * point is read from there — a file that lives in the vault is linked where it
 * is, and never copied alongside itself.
 */
export async function insertFileAtCursor(path: string, target: AttachmentTarget): Promise<void> {
	try {
		await insertDroppedPath(path, target, captureInsertionPoint(target.editor));
	} catch (err) {
		toast.error(m.toast_insert_file_failed({ error: String(err) }));
	}
}

/**
 * Insert a set of OS-dropped paths at `position`.
 *
 * The OS-drop router has already hit-tested the drop onto this editor and
 * converted the physical position Tauri reports into CSS pixels, so the only
 * jobs left here are to place the caret and to insert — at the position read
 * once, before the first import round-trip.
 */
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
