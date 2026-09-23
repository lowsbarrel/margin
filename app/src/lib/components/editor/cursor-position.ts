import type { Editor } from '@tiptap/core';
import { editor as editorStore } from '$lib/stores/editor.svelte';

export function reportRichCursor(editor: Editor): void {
	const { doc, selection } = editor.state;
	const from = selection.from;
	const resolved = doc.resolve(from);
	let line = resolved.index(0) + 1;
	let col = 1;
	if (resolved.depth >= 1) {
		const blockStart = resolved.start(1);
		col = from - blockStart + 1;
		if (from > blockStart) {
			const blockText = doc.textBetween(blockStart, from, '\n');
			const newline = blockText.lastIndexOf('\n');
			if (newline !== -1) {
				line += blockText.split('\n').length - 1;
				col = blockText.length - newline;
			}
		}
	}
	editorStore.setCursor(line, col);
}

export function restoreRichCursor(editor: Editor, position: number): void {
	const size = editor.state.doc.content.size;
	const pos = Math.min(Math.max(position, 0), Math.max(size - 1, 0));
	try {
		editor.commands.setTextSelection(pos);
		editor.commands.scrollIntoView();
	} catch {}
}

export function richCursorToSourceOffset(editor: Editor, sourceText: string): number {
	const { doc, selection } = editor.state;
	const from = selection.from;
	const proportional = Math.round((from / Math.max(doc.content.size, 1)) * sourceText.length);
	const resolved = doc.resolve(from);
	if (resolved.depth < 1) return proportional;
	const anchor = doc.textBetween(resolved.start(1), from, ' ').slice(-24).trim();
	if (!anchor) return proportional;
	const at = sourceText.indexOf(anchor);
	return at === -1 ? proportional : at + anchor.length;
}

export function sourceOffsetToRichPos(editor: Editor, sourceText: string, offset: number): number {
	const doc = editor.state.doc;
	const size = doc.content.size;
	const clamp = (pos: number) => Math.min(Math.max(pos, 0), Math.max(size - 1, 0));
	const proportional = Math.round((offset / Math.max(sourceText.length, 1)) * size);
	const anchor = sourceText.slice(Math.max(0, offset - 24), offset).trim();
	if (!anchor) return clamp(proportional);
	const plain = doc.textBetween(0, size, '\n');
	const at = plain.indexOf(anchor);
	if (at === -1) return clamp(proportional);
	return clamp(Math.round(((at + anchor.length) / Math.max(plain.length, 1)) * size));
}

export function reportSourceCursor(text: string, offset: number): void {
	const lineStart = text.lastIndexOf('\n', Math.max(offset - 1, 0));
	const line = text.slice(0, offset).split('\n').length;
	editorStore.setCursor(line, offset - lineStart);
}
