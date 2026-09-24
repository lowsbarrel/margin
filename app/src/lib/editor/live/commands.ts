import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdownKeymap } from '@codemirror/lang-markdown';
import type { ChangeSpec, EditorState, Line } from '@codemirror/state';
import { keymap, type EditorView } from '@codemirror/view';

const HEADING = /^(#{1,6})[ \t]+/;
const MARKER = /^([ \t]*)(?:([-*+])|(\d+[.)]))([ \t]+)(\[[ xX]\][ \t]+)?/;
const LIST_ITEM = /^[ \t]*(?:[-*+]|\d+[.)])[ \t]/;

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

function toggleList(view: EditorView, kind: ListKind): boolean {
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

function toggleMark(view: EditorView, marker: string): boolean {
	const { state } = view;
	const range = state.selection.main;
	if (range.empty) {
		view.dispatch({
			changes: { from: range.from, insert: marker + marker },
			selection: { anchor: range.from + marker.length },
			userEvent: 'input'
		});
		return true;
	}
	const { from, to } = range;
	const text = state.sliceDoc(from, to);
	const outer = marker.length;
	if (text.length > outer * 2 && text.startsWith(marker) && text.endsWith(marker)) {
		view.dispatch({
			changes: [
				{ from, to: from + outer },
				{ from: to - outer, to }
			],
			selection: { anchor: from, head: to - outer * 2 },
			userEvent: 'input'
		});
		return true;
	}
	if (
		from >= outer &&
		state.sliceDoc(from - outer, from) === marker &&
		state.sliceDoc(to, to + outer) === marker
	) {
		view.dispatch({
			changes: [
				{ from: from - outer, to: from },
				{ from: to, to: to + outer }
			],
			selection: { anchor: from - outer, head: from - outer + text.length },
			userEvent: 'input'
		});
		return true;
	}
	view.dispatch({
		changes: [
			{ from, insert: marker },
			{ from: to, insert: marker }
		],
		selection: { anchor: from, head: to + outer * 2 },
		userEvent: 'input'
	});
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
	{ key: 'Mod-b', run: (view) => toggleMark(view, '**'), preventDefault: true },
	{ key: 'Mod-i', run: (view) => toggleMark(view, '*'), preventDefault: true },
	{ key: 'Mod-e', run: (view) => toggleMark(view, '`'), preventDefault: true },
	{ key: 'Mod-Shift-x', run: (view) => toggleMark(view, '~~'), preventDefault: true },
	{ key: 'Mod-Shift-h', run: (view) => toggleMark(view, '=='), preventDefault: true },
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
	...markdownKeymap,
	...defaultKeymap,
	...historyKeymap
]);

export const liveHistory = history();
