import type { Editor, JSONContent } from '@tiptap/core';
import { NodeSelection } from '@tiptap/pm/state';
import { importAttachment, storeAttachmentBytes } from '$lib/fs/bridge';
import { isImageFile } from '$lib/utils/mime';
import { buildLocalfileUrl } from '$lib/editor/image-url';

// Not dot-prefixed: the watcher, sync and the zip export all skip hidden paths.
export const DEFAULT_ATTACHMENT_FOLDER = 'attachments';

export function resolveAttachmentFolder(setting: string | null | undefined): string {
	return setting?.trim() || DEFAULT_ATTACHMENT_FOLDER;
}

export interface AttachmentTarget {
	editor: Editor;
	vaultPath: string;
	attachmentFolder: string;
}

export interface InsertionPoint {
	from: number;
	to: number;
}

export function captureInsertionPoint(editor: Editor): InsertionPoint {
	const { from, to } = editor.state.selection;
	// TipTap selects the atom it just inserted; inserting *at* that range would consume it.
	if (editor.state.selection instanceof NodeSelection) return { from: to, to };
	return { from, to };
}

export function insertNodeAt(
	editor: Editor,
	point: InsertionPoint,
	node: JSONContent
): InsertionPoint {
	const size = editor.state.doc.content.size;
	const range = { from: Math.min(point.from, size), to: Math.min(point.to, size) };
	editor.chain().focus().setTextSelection(range).insertContentAt(range, node).run();
	const at = editor.state.selection.to;
	return { from: at, to: at };
}

function imageNode(relPath: string, alt: string, vaultPath: string): JSONContent {
	return {
		type: 'image',
		attrs: { src: buildLocalfileUrl(`${vaultPath}/${relPath}`), alt }
	};
}

function embedNode(relPath: string, target: string): JSONContent {
	return { type: 'fileEmbed', attrs: { src: relPath, filename: target } };
}

function fileName(relPath: string): string {
	return relPath.slice(relPath.lastIndexOf('/') + 1);
}

export function vaultRelativePath(path: string, vaultPath: string): string | null {
	const trimSlashes = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '');
	const target = trimSlashes(path);
	const root = trimSlashes(vaultPath);
	return target.startsWith(`${root}/`) ? target.slice(root.length + 1) : null;
}

export async function insertPastedFile(
	file: File,
	{ editor, vaultPath, attachmentFolder }: AttachmentTarget,
	at: InsertionPoint
): Promise<InsertionPoint> {
	const bytes = new Uint8Array(await file.arrayBuffer());
	const relPath = await storeAttachmentBytes(attachmentFolder, file.name, bytes);
	const node = isImageFile(file.name, file.type)
		? imageNode(relPath, file.name, vaultPath)
		: embedNode(relPath, fileName(relPath));
	return insertNodeAt(editor, at, node);
}

export async function insertDroppedPath(
	source: string,
	{ editor, vaultPath, attachmentFolder }: AttachmentTarget,
	at: InsertionPoint
): Promise<InsertionPoint> {
	const name = source.replace(/\\/g, '/').split('/').pop() ?? source;
	const ext = name.split('.').pop()?.toLowerCase() ?? '';

	if (ext === 'md' || ext === 'canvas') {
		const title = name.slice(0, -(ext.length + 1));
		return insertNodeAt(editor, at, { type: 'wikiLink', attrs: { title } });
	}

	const linked = vaultRelativePath(source, vaultPath);
	const relPath = linked ?? (await importAttachment(source, attachmentFolder));
	if (isImageFile(name)) {
		return insertNodeAt(editor, at, imageNode(relPath, name, vaultPath));
	}
	return insertNodeAt(editor, at, embedNode(relPath, linked ?? fileName(relPath)));
}
