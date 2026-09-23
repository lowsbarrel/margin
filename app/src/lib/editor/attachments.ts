import type { Editor, JSONContent } from '@tiptap/core';
import { NodeSelection } from '@tiptap/pm/state';
import { importAttachment, storeAttachmentBytes } from '$lib/fs/bridge';
import { isImageFile } from '$lib/utils/mime';
import { buildLocalfileUrl } from '$lib/editor/image-url';

/**
 * Attachments are one vault-level folder. It is never dot-prefixed: every
 * walker, the watcher, sync and the zip export all skip hidden paths, so a
 * `.attachments` would silently fall out of all three.
 */
export const DEFAULT_ATTACHMENT_FOLDER = 'attachments';

/** The folder pasted and dropped files land in. */
export function resolveAttachmentFolder(setting: string | null | undefined): string {
	return setting?.trim() || DEFAULT_ATTACHMENT_FOLDER;
}

export interface AttachmentTarget {
	editor: Editor;
	vaultPath: string;
	attachmentFolder: string;
}

/** A known place in the document, held while an IPC round-trip is in flight. */
export interface InsertionPoint {
	from: number;
	to: number;
}

/**
 * Where the node must end up, captured before the first await: storing a file
 * is an IPC round-trip, and inserting afterwards would drop the node wherever
 * the caret happened to be by then instead of at the paste or the drop.
 */
export function captureInsertionPoint(editor: Editor): InsertionPoint {
	const { from, to } = editor.state.selection;
	// TipTap selects whatever atom it just inserted. That selection is not a
	// range the user asked to replace — inserting *at* it would consume the image
	// that arrived a moment ago — so it collapses to the far side of the node.
	if (editor.state.selection instanceof NodeSelection) return { from: to, to };
	return { from, to };
}

/** Insert one node at `point` and return the point just after it. */
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

/** A picture keeps the name the user knows. */
function imageNode(relPath: string, alt: string, vaultPath: string): JSONContent {
	return {
		type: 'image',
		attrs: { src: buildLocalfileUrl(`${vaultPath}/${relPath}`), alt }
	};
}

/**
 * A file embed. `target` is the label and, on load, the thing that resolves: a
 * bare name lands in the attachments folder, so a file that lives anywhere else
 * has to carry its path.
 */
function embedNode(relPath: string, target: string): JSONContent {
	return { type: 'fileEmbed', attrs: { src: relPath, filename: target } };
}

/** The file's own name — what a bare embed target resolves back to. */
function fileName(relPath: string): string {
	return relPath.slice(relPath.lastIndexOf('/') + 1);
}

/** The vault-relative path of `path` when it is inside the vault, else null. */
export function vaultRelativePath(path: string, vaultPath: string): string | null {
	const trimSlashes = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '');
	const target = trimSlashes(path);
	const root = trimSlashes(vaultPath);
	return target.startsWith(`${root}/`) ? target.slice(root.length + 1) : null;
}

/**
 * Store a pasted file and insert it at `at`, returning the next position.
 */
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

/**
 * Insert one path the user dropped or dragged in, at `at`.
 *
 * A note becomes a wiki link; a file that is already in the vault is linked
 * where it is — copying it would duplicate every image the tree hands over —
 * and only a path from outside the vault is imported into the attachments
 * folder first.
 */
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
	// A linked file keeps its path — a bare name would resolve to the attachments
	// folder on load; a stored one is named by the store, so its bare name fits.
	return insertNodeAt(editor, at, embedNode(relPath, linked ?? fileName(relPath)));
}
