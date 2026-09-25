import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import type { EditorState } from '@codemirror/state';
import { ViewPlugin, type EditorView } from '@codemirror/view';
import type TurndownService from 'turndown';
import { insertPastedFiles } from './attachments';
import { captureSelection, insertText } from './insert';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';

const CODE_NODES: Record<string, true> = { FencedCode: true, CodeBlock: true, InlineCode: true };
const CODE_VIEW_MARKERS =
	/monaco-editor|cm-editor|CodeMirror|blob-code|blob-wrapper|highlight-source|linenumbers|line-numbers|hljs/;
const MONOSPACE = /monospace|consolas|courier|menlo|monaco|source code|jetbrains mono|sf mono/i;

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

// A code editor's clipboard HTML is a syntax-highlighted dump: turning it into Markdown would mangle the source.
function isCodeEditorHtml(html: string): boolean {
	const body = new DOMParser().parseFromString(html, 'text/html').body;
	const root = body.firstElementChild;
	if (root) {
		if (/white-space\s*:\s*(pre|pre-wrap|pre-line)/.test(root.getAttribute('style') ?? ''))
			return true;
		if (body.children.length === 1 && root.tagName === 'PRE') return true;
	}
	if (CODE_VIEW_MARKERS.test(html)) return true;
	const runs = [...body.querySelectorAll('*')].filter(
		(el) => el.children.length === 0 && (el.textContent ?? '').trim().length > 0
	);
	if (runs.length === 0) return false;
	return runs.every((el) => {
		if (el.tagName === 'CODE' || el.closest('pre')) return true;
		return (
			MONOSPACE.test(el.getAttribute('style') ?? '') ||
			MONOSPACE.test(el.parentElement?.getAttribute('style') ?? '')
		);
	});
}

const BLOCK_LEAD = /^([ \t]{0,3})(-{2,}|={2,}|#{1,6}|[-+*]|\d+[.)]|>)(?=[ \t]|$)/gm;

function escapeMarkdown(text: string): string {
	return text.replace(/\\/g, '\\\\').replace(BLOCK_LEAD, '$1\\$2');
}

function listItemRule(
	content: string,
	item: HTMLElement,
	options: TurndownService.Options
): string {
	const indented = content.replace(/^\n+/, '').replace(/\n+$/, '\n').replace(/\n/gm, '\n    ');
	const parent = item.parentNode as HTMLElement | null;
	let prefix = `${options.bulletListMarker} `;
	if (parent?.nodeName === 'OL') {
		const start = parent.getAttribute('start');
		const index = Array.prototype.indexOf.call(parent.children, item);
		prefix = `${start ? Number(start) + index : index + 1}. `;
	}
	return prefix + indented + (item.nextSibling && !/\n$/.test(indented) ? '\n' : '');
}

async function htmlToMarkdown(html: string): Promise<string> {
	const [{ default: TurndownService }, { gfm }] = await Promise.all([
		import('turndown'),
		import('turndown-plugin-gfm')
	]);
	const service = new TurndownService({
		headingStyle: 'atx',
		hr: '---',
		bulletListMarker: '-',
		codeBlockStyle: 'fenced',
		emDelimiter: '*',
		strongDelimiter: '**'
	});
	service.use(gfm);
	service.addRule('listItem', { filter: 'li', replacement: listItemRule });
	service.escape = escapeMarkdown;
	return service.turndown(html);
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
				insertText(view, cursor, await htmlToMarkdown(html));
				return;
			}
			const files = await clipboardImages(items);
			if (files.length > 0) {
				await insertPastedFiles(view, files, cursor);
				return;
			}
		}
		const text = await navigator.clipboard.readText();
		if (text) insertText(view, cursor, text);
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
				if (text) insertText(view, captureSelection(view), text);
			})
			.catch(() => {});
	}, 250);
}

function handlePasteEvent(view: EditorView, event: ClipboardEvent): boolean {
	const data = event.clipboardData;
	if (!data) return false;
	if (insideCode(view.state)) return false;

	const forcedPlain = plainNext;
	plainNext = false;
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
		insertText(view, captureSelection(view), data.getData('text/plain'));
		return true;
	}

	event.preventDefault();
	const cursor = captureSelection(view);
	void htmlToMarkdown(html).then((markdown) => insertText(view, cursor, markdown));
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
