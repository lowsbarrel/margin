import { defaultKeymap, historyKeymap } from '@codemirror/commands';
import { markdownKeymap } from '@codemirror/lang-markdown';
import { syntaxTree } from '@codemirror/language';
import { toggleMark } from './marks';
import type { ChangeSpec, EditorState, Line } from '@codemirror/state';
import { keymap, type EditorView } from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';
import { alignColumn, deleteColumn, deleteRow, insertColumn, insertRow } from './table-ops';
import { tableAt, type Align, type TableModel } from './table-model';

const HEADING = /^(#{1,6})[ \t]+/;
const MARKER = /^([ \t]*)(?:([-*+])|(\d+[.)]))([ \t]+)(\[[ xX]\][ \t]+)?/;
const LIST_ITEM = /^[ \t]*(?:[-*+]|\d+[.)])[ \t]/;
const EMPTY_QUOTE = /^[ \t]*(?:>[ \t]?)+$/;

function exitEmptyQuote(view: EditorView): boolean {
	const range = view.state.selection.main;
	if (!range.empty) return false;
	const line = view.state.doc.lineAt(range.head);
	if (range.head !== line.to || !EMPTY_QUOTE.test(line.text)) return false;
	// A line right after a quote is a lazy continuation, so leaving it needs a blank line.
	view.dispatch({
		changes: { from: line.from, to: line.to, insert: '\n' },
		selection: { anchor: line.from + 1 },
		scrollIntoView: true
	});
	return true;
}

type ListKind = 'bullet' | 'ordered' | 'task';

function selectedLines(state: EditorState): Line[] {
	const range = state.selection.main;
	const first = state.doc.lineAt(range.from);
	const last = state.doc.lineAt(range.to);
	const lines: Line[] = [];
	for (let number = first.number; number <= last.number; number++)
		lines.push(state.doc.line(number));
	return lines;
}

function toggleHeading(view: EditorView, level: number): boolean {
	const changes: ChangeSpec[] = [];
	for (const line of selectedLines(view.state)) {
		const match = HEADING.exec(line.text);
		const prefix = `${'#'.repeat(level)} `;
		if (!match) changes.push({ from: line.from, insert: prefix });
		else if (match[1].length === level)
			changes.push({ from: line.from, to: line.from + match[0].length, insert: '' });
		else changes.push({ from: line.from, to: line.from + match[0].length, insert: prefix });
	}
	if (!changes.length) return false;
	view.dispatch({ changes, userEvent: 'input' });
	return true;
}

export function toggleList(view: EditorView, kind: ListKind): boolean {
	const changes: ChangeSpec[] = [];
	for (const line of selectedLines(view.state)) {
		const match = MARKER.exec(line.text);
		const marker = kind === 'ordered' ? '1.' : '-';
		const prefix = kind === 'task' ? `${marker} [ ] ` : `${marker} `;
		if (!match) {
			changes.push({ from: line.from, insert: prefix });
			continue;
		}
		const current: ListKind = match[2] ? 'bullet' : 'ordered';
		const isTask = Boolean(match[5]);
		const to = line.from + match[0].length;
		const same = kind === 'task' ? isTask : current === kind && !isTask;
		changes.push({ from: line.from, to, insert: same ? match[1] : `${match[1]}${prefix}` });
	}
	if (!changes.length) return false;
	view.dispatch({ changes, userEvent: 'input' });
	return true;
}

const QUOTE_PREFIX = /^[ \t]*>[ \t]?/;

export function toggleQuote(view: EditorView): boolean {
	const lines = selectedLines(view.state);
	const quoted = lines.every((line) => QUOTE_PREFIX.test(line.text));
	const changes: ChangeSpec[] = [];
	for (const line of lines) {
		const match = QUOTE_PREFIX.exec(line.text);
		if (!quoted) changes.push({ from: line.from, insert: '> ' });
		else if (match) changes.push({ from: line.from, to: line.from + match[0].length, insert: '' });
	}
	if (!changes.length) return false;
	view.dispatch({ changes, userEvent: 'input' });
	return true;
}

export type BlockType =
	| 'text'
	| 'heading1'
	| 'heading2'
	| 'heading3'
	| 'heading4'
	| 'heading5'
	| 'heading6'
	| 'bullet'
	| 'ordered'
	| 'task'
	| 'quote'
	| 'code';

const BLOCK_PREFIX: Partial<Record<BlockType, string>> = {
	text: '',
	heading1: '# ',
	heading2: '## ',
	heading3: '### ',
	heading4: '#### ',
	heading5: '##### ',
	heading6: '###### ',
	bullet: '- ',
	ordered: '1. ',
	task: '- [ ] ',
	quote: '> '
};
const NESTED: Record<string, true> = { bullet: true, ordered: true, task: true };
const FENCE_LINE = /^[ \t]*(?:```|~~~)/;

interface Fence {
	from: number;
	to: number;
}

export function fencedRange(state: EditorState): Fence | null {
	let node: SyntaxNode | null = syntaxTree(state).resolveInner(state.selection.main.head, -1);
	while (node) {
		if (node.name === 'FencedCode') return { from: node.from, to: node.to };
		node = node.parent;
	}
	return null;
}

function stripBlock(text: string): string {
	let rest = text;
	for (const pattern of [HEADING, QUOTE_PREFIX]) {
		const match = pattern.exec(rest);
		if (match) rest = rest.slice(match[0].length);
	}
	const marker = MARKER.exec(rest);
	return marker ? marker[1] + rest.slice(marker[0].length) : rest;
}

function blockLine(type: BlockType, text: string): string {
	const body = stripBlock(text);
	const prefix = BLOCK_PREFIX[type] ?? '';
	if (!prefix) return body;
	const indent = NESTED[type] ? (/^[ \t]*/.exec(body)?.[0] ?? '') : '';
	return indent + prefix + body.slice(indent.length);
}

function fencePlan(state: EditorState, type: BlockType, lines: Line[]) {
	const fence = fencedRange(state);
	if (fence) return unwrapPlan(state, fence);
	if (type !== 'code')
		return { before: null, after: null, selection: undefined, openFrom: -1, closeFrom: -1 };
	const first = lines[0];
	const last = lines[lines.length - 1];
	return {
		before: { from: first.from, insert: '```\n' },
		after: { from: last.to, insert: '\n```' },
		selection: { anchor: first.from, head: last.to + 8 },
		openFrom: -1,
		closeFrom: -1
	};
}

function unwrapPlan(state: EditorState, fence: Fence) {
	const open = state.doc.lineAt(fence.from);
	const close = state.doc.lineAt(Math.max(open.from + 1, fence.to - 1));
	const closed = close.number > open.number && FENCE_LINE.test(close.text);
	const openLength = Math.min(open.to + 1, state.doc.length) - open.from;
	return {
		before: { from: open.from, to: open.from + openLength },
		after: closed ? { from: close.from - 1, to: close.to } : null,
		openFrom: open.from,
		closeFrom: closed ? close.from : -1,
		selection: {
			anchor: open.from,
			head: Math.max(open.from, (closed ? close.from - 1 : fence.to) - openLength)
		}
	};
}

export function setBlock(view: EditorView, type: BlockType): boolean {
	const state = view.state;
	const lines = selectedLines(state);
	const plan = fencePlan(state, type, lines);
	const changes: ChangeSpec[] = [];
	if (plan.before) changes.push(plan.before);
	if (type !== 'code') {
		for (const line of lines) {
			if (line.from === plan.openFrom || line.from === plan.closeFrom) continue;
			const next = blockLine(type, line.text);
			if (next !== line.text) changes.push({ from: line.from, to: line.to, insert: next });
		}
	}
	if (plan.after) changes.push(plan.after);
	if (!changes.length) return false;
	view.dispatch({ changes, selection: plan.selection, userEvent: 'input' });
	return true;
}

function insertLink(view: EditorView): boolean {
	const range = view.state.selection.main;
	const text = view.state.sliceDoc(range.from, range.to);
	const insert = `[${text}]()`;
	view.dispatch({
		changes: { from: range.from, to: range.to, insert },
		selection: { anchor: range.from + text.length + 3 },
		userEvent: 'input'
	});
	return true;
}

const EXISTING_LINK = /^\[([^\]]*)\]\([^)]*\)$/;

export function setLink(view: EditorView, url: string): boolean {
	const range = view.state.selection.main;
	const text = view.state.sliceDoc(range.from, range.to);
	const existing = EXISTING_LINK.exec(text);
	const insert = `[${existing?.[1] ?? text}](${url})`;
	view.dispatch({
		changes: { from: range.from, to: range.to, insert },
		selection: { anchor: range.from + insert.length },
		userEvent: 'input'
	});
	return true;
}

function indentList(view: EditorView, outdent: boolean): boolean {
	const changes: ChangeSpec[] = [];
	for (const line of selectedLines(view.state)) {
		if (!LIST_ITEM.test(line.text)) continue;
		if (!outdent) {
			changes.push({ from: line.from, insert: '  ' });
			continue;
		}
		const indent = /^[ \t]{1,2}/.exec(line.text);
		if (indent) changes.push({ from: line.from, to: line.from + indent[0].length, insert: '' });
	}
	if (!changes.length) return false;
	view.dispatch({ changes, userEvent: 'input' });
	return true;
}

export const liveKeymap = keymap.of([
	{ key: 'Mod-b', run: (view) => toggleMark(view, 'bold'), preventDefault: true },
	{ key: 'Mod-i', run: (view) => toggleMark(view, 'italic'), preventDefault: true },
	{ key: 'Mod-e', run: (view) => toggleMark(view, 'code'), preventDefault: true },
	{ key: 'Mod-Shift-x', run: (view) => toggleMark(view, 'strike'), preventDefault: true },
	{ key: 'Mod-Shift-h', run: (view) => toggleMark(view, 'highlight'), preventDefault: true },
	{ key: 'Mod-k', run: insertLink, preventDefault: true },
	{ key: 'Mod-Shift-7', run: (view) => toggleList(view, 'ordered') },
	{ key: 'Mod-Shift-8', run: (view) => toggleList(view, 'bullet') },
	{ key: 'Mod-Shift-9', run: (view) => toggleList(view, 'task') },
	{ key: 'Mod-1', run: (view) => toggleHeading(view, 1) },
	{ key: 'Mod-2', run: (view) => toggleHeading(view, 2) },
	{ key: 'Mod-3', run: (view) => toggleHeading(view, 3) },
	{ key: 'Mod-4', run: (view) => toggleHeading(view, 4) },
	{ key: 'Mod-5', run: (view) => toggleHeading(view, 5) },
	{ key: 'Mod-6', run: (view) => toggleHeading(view, 6) },
	{ key: 'Tab', run: (view) => indentList(view, false) },
	{ key: 'Shift-Tab', run: (view) => indentList(view, true) },
	{ key: 'Enter', run: exitEmptyQuote },
	...markdownKeymap,
	...defaultKeymap,
	...historyKeymap
]);

function tableCommand(
	view: EditorView,
	anchor: number,
	action: (table: TableModel) => void
): boolean {
	const table = tableAt(view.state, anchor);
	if (!table) return false;
	action(table);
	return true;
}

// The slash menu and the editor context menu reach tables through these five, keyed by caret.
export function insertTableRow(
	view: EditorView,
	index: number,
	side: 'above' | 'below',
	anchor: number = view.state.selection.main.head
): boolean {
	return tableCommand(view, anchor, (table) => insertRow(view, table, index, side));
}

export function deleteTableRow(
	view: EditorView,
	index: number,
	anchor: number = view.state.selection.main.head
): boolean {
	return tableCommand(view, anchor, (table) => deleteRow(view, table, index));
}

export function insertTableColumn(
	view: EditorView,
	index: number,
	side: 'left' | 'right',
	anchor: number = view.state.selection.main.head
): boolean {
	return tableCommand(view, anchor, (table) => insertColumn(view, table, index, side));
}

export function deleteTableColumn(
	view: EditorView,
	index: number,
	anchor: number = view.state.selection.main.head
): boolean {
	return tableCommand(view, anchor, (table) => deleteColumn(view, table, index));
}

export function alignTableColumn(
	view: EditorView,
	index: number,
	align: Align,
	anchor: number = view.state.selection.main.head
): boolean {
	return tableCommand(view, anchor, (table) => alignColumn(view, table, index, align));
}
