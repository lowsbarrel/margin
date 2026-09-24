import type { EditorView } from '@codemirror/view';
import { importAttachment, storeAttachmentBytes } from '$lib/fs/bridge';
import { isImageFile } from '$lib/utils/mime';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';
import { contextOf } from './context';
import { captureSelection, insertText, type TextRange } from './insert';
import { encodeDestination, vaultRelative } from './resolve';

function linkFor(relPath: string, name: string, image: boolean): string {
	const destination = encodeDestination(relPath);
	return image ? `![](${destination})` : `[${name}](${destination})`;
}

function noteTitle(name: string): string {
	return name.replace(/\.(md|canvas)$/i, '');
}

export async function insertDroppedPaths(view: EditorView, paths: string[]): Promise<void> {
	const ctx = contextOf(view.state);
	const vaultPath = ctx.vaultPath();
	if (!vaultPath) return;
	let cursor: TextRange = captureSelection(view);
	for (const source of paths) {
		const name = source.replace(/\\/g, '/').split('/').pop() ?? source;
		const ext = name.split('.').pop()?.toLowerCase() ?? '';
		try {
			if (ext === 'md' || ext === 'canvas') {
				cursor = insertText(view, cursor, `[[${noteTitle(name)}]]`);
				continue;
			}
			const linked = vaultRelative(source, vaultPath);
			const abs = linked
				? `${vaultPath}/${linked}`
				: await importAttachment(source, ctx.attachmentFolder());
			const rel = vaultRelative(abs, vaultPath);
			if (!rel) continue;
			cursor = insertText(view, cursor, linkFor(rel, name, isImageFile(name)));
		} catch (err) {
			toast.error(m.toast_insert_file_failed({ error: String(err) }));
		}
	}
}

export async function insertPastedFiles(
	view: EditorView,
	files: File[],
	from: TextRange = captureSelection(view)
): Promise<void> {
	const ctx = contextOf(view.state);
	const vaultPath = ctx.vaultPath();
	if (!vaultPath) return;
	let cursor: TextRange = from;
	for (const file of files) {
		try {
			const bytes = new Uint8Array(await file.arrayBuffer());
			const abs = await storeAttachmentBytes(ctx.attachmentFolder(), file.name, bytes);
			const rel = vaultRelative(abs, vaultPath);
			if (!rel) continue;
			cursor = insertText(view, cursor, linkFor(rel, file.name, isImageFile(file.name, file.type)));
		} catch (err) {
			toast.error(m.toast_attachment_paste_failed({ name: file.name, error: String(err) }));
		}
	}
}
