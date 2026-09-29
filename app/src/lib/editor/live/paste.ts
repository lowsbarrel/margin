import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import { EditorSelection, type EditorState } from '@codemirror/state';
import { ViewPlugin, type EditorView } from '@codemirror/view';
import { insertPastedFiles } from './attachments';
import { captureSelection, insertText, type TextRange } from './insert';
import { htmlToMarkdown, isCodeEditorHtml } from './paste-html';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';
import { markdownLink, WEB_URL } from '$lib/utils/web-link';

const CODE_NODES: Record<string, true> = { FencedCode: true, CodeBlock: true, InlineCode: true };
const BARE_MARKER = /^([ \t]*)(?:[-+*]|\d+[.)])(?: \[[ xX]\])? $/;
const LIST_START = /^[ \t]*(?:[-+*]|\d+[.)])[ \t]/;

let plainNext = false;

function insideCode(state: EditorState): boolean {
	let node: SyntaxNode | null = syntaxTree(state).resolveInner(state.selection.main.head, -1);
	while (node) {
		if (CODE_NODES[node.name]) return true;
		node = node.parent;
	}
	return false;
}

function clipboardFiles(data: DataTransfer): File[] {
	const files: File[] = [];
	for (let i = 0; i < data.items.length; i++) {
		const item = data.items[i];
		if (item.kind !== 'file') continue;
		const file = item.getAsFile();
		if (file) files.push(file);
	}
	if (files.length === 0) {
		for (let i = 0; i < data.files.length; i++) files.push(data.files[i]);
	}
	return files;
}

// Pasted list items replace a bare marker on the caret's line and nest under its indentation.
function pasteText(view: EditorView, range: TextRange, raw: string): void {
	const text = raw.replace(/\r\n?/g, '\n');
	const line = view.state.doc.lineAt(range.from);
	const bare = BARE_MARKER.exec(view.state.sliceDoc(line.from, range.from));
	const rest = range.to <= line.to ? view.state.sliceDoc(range.to, line.to) : 'x';
	if (!bare || rest.trim() || !LIST_START.test(text)) {
		insertText(view, range, text);
		return;
	}
	const indent = bare[1];
	const lines = text.split('\n');
	const base = /^[ \t]*/.exec(lines[0])?.[0] ?? '';
	const fitted = lines.map((entry, i) => {
		const own = entry.startsWith(base) ? entry.slice(base.length) : entry.trimStart();
		return i === 0 || !own ? own : indent + own;
	});
	insertText(view, { from: line.from + indent.length, to: range.to }, fitted.join('\n'));
}

async function clipboardImages(items: ClipboardItems): Promise<File[]> {
	const files: File[] = [];
	for (const item of items) {
		const type = item.types.find((entry) => entry.startsWith('image/'));
		if (!type) continue;
		const blob = await item.getType(type);
		const ext = type.split('/')[1]?.split('+')[0] || 'png';
		files.push(new File([blob], `pasted-${Date.now()}.${ext}`, { type }));
	}
	return files;
}

async function readClipboardImages(): Promise<File[]> {
	try {
		return await clipboardImages(await navigator.clipboard.read());
	} catch (err) {
		console.error('Clipboard image read failed:', err);
		return [];
	}
}

async function clipboardHtml(items: ClipboardItems): Promise<string | null> {
	for (const item of items) {
		if (!item.types.includes('text/html')) continue;
		return (await item.getType('text/html')).text();
	}
	return null;
}

export async function pasteFromClipboard(view: EditorView, plain: boolean): Promise<void> {
	const cursor = captureSelection(view);
	try {
		if (!plain) {
			const items = await navigator.clipboard.read();
			const html = await clipboardHtml(items);
			if (html && !isCodeEditorHtml(html)) {
				pasteText(view, cursor, await htmlToMarkdown(html));
				return;
			}
			const files = await clipboardImages(items);
			if (files.length > 0) {
				await insertPastedFiles(view, files, cursor);
				return;
			}
		}
		const text = await navigator.clipboard.readText();
		if (text) pasteText(view, cursor, text);
	} catch (err) {
		toast.error(m.toast_clipboard_paste_failed({ error: String(err) }));
	}
}

function armPlainPaste(view: EditorView): void {
	plainNext = true;
	window.setTimeout(() => {
		if (!plainNext) return;
		plainNext = false;
		void navigator.clipboard
			.readText()
			.then((text) => {
				if (text) pasteText(view, captureSelection(view), text);
			})
			.catch(() => {});
	}, 250);
}

function linkSelection(view: EditorView, url: string): boolean {
	const { state } = view;
	const ranges = state.selection.ranges;
	if (ranges.some((range) => range.empty || state.doc.lineAt(range.from).to < range.to))
		return false;
	view.dispatch(
		state.changeByRange((range) => {
			const insert = markdownLink(state.sliceDoc(range.from, range.to), url);
			return {
				changes: { from: range.from, to: range.to, insert },
				range: EditorSelection.cursor(range.from + insert.length)
			};
		}),
		{ userEvent: 'input.paste', scrollIntoView: true }
	);
	return true;
}

function handlePasteEvent(view: EditorView, event: ClipboardEvent): boolean {
	const data = event.clipboardData;
	if (!data) return false;
	if (insideCode(view.state)) return false;

	const forcedPlain = plainNext;
	plainNext = false;
	const url = data.getData('text/plain').trim();
	if (!forcedPlain && WEB_URL.test(url) && linkSelection(view, url)) {
		event.preventDefault();
		return true;
	}
	const files = clipboardFiles(data);
	const hasImageType =
		files.length === 0 && Array.from(data.types ?? []).some((type) => type.startsWith('image/'));

	if (files.length > 0 || hasImageType) {
		event.preventDefault();
		// Read before the first await: the caret may move while the async clipboard read is in flight.
		const cursor = captureSelection(view);
		void (async () => {
			const list = files.length > 0 ? files : await readClipboardImages();
			if (list.length > 0) await insertPastedFiles(view, list, cursor);
		})();
		return true;
	}

	const html = data.getData('text/html');
	if (forcedPlain || !html || isCodeEditorHtml(html)) {
		event.preventDefault();
		pasteText(view, captureSelection(view), data.getData('text/plain'));
		return true;
	}

	event.preventDefault();
	const cursor = captureSelection(view);
	void htmlToMarkdown(html).then((markdown) => pasteText(view, cursor, markdown));
	return true;
}

export const livePaste = ViewPlugin.fromClass(
	class {
		constructor(readonly view: EditorView) {}
	},
	{
		eventHandlers: {
			keydown(event) {
				if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'v') {
					armPlainPaste(this.view);
				}
				return false;
			},
			paste(event, view) {
				return handlePasteEvent(view, event as ClipboardEvent);
			}
		}
	}
);
