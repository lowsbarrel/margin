import { syntaxTree } from '@codemirror/language';
import { EditorSelection, type ChangeSpec, type EditorState } from '@codemirror/state';
import type { EditorView, KeyBinding } from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';

const PREFIX = /^([ \t]*)((?:>[ \t]?)*)([ \t]*)(?:([-*+]|\d{1,9}[.)])([ \t]+)(\[[ xX]\][ \t]+)?)?/;

function prefixOf(text: string) {
	const [whole, indent, quotes, inner, marker, gap, box] = PREFIX.exec(text)!;
	return { whole, head: indent + quotes + inner, marker, gap, box };
}

function inCode(state: EditorState, pos: number): boolean {
	let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, -1);
	for (; node; node = node.parent) if (node.name === 'FencedCode') return true;
	return false;
}

function contentStart(view: EditorView, head: number): number | null {
	const { state } = view;
	const line = state.doc.lineAt(head);
	if (inCode(state, line.from)) return null;
	const start = line.from + prefixOf(line.text).whole.length;
	if (start === line.from) return null;
	const row = view.moveToLineBoundary(EditorSelection.cursor(head), false, true).head;
	if (row > start) return null;
	return head > start ? start : line.from;
}

function toContentStart(view: EditorView, extend: boolean): boolean {
	const { state } = view;
	const targets = state.selection.ranges.map((range) => contentStart(view, range.head));
	if (targets.every((target) => target === null)) return false;
	const ranges = state.selection.ranges.map((range, i) => {
		const target =
			targets[i] ?? view.moveToLineBoundary(EditorSelection.cursor(range.head), false, true).head;
		return extend ? EditorSelection.range(range.anchor, target) : EditorSelection.cursor(target);
	});
	view.dispatch({
		selection: EditorSelection.create(ranges, state.selection.mainIndex),
		scrollIntoView: true,
		userEvent: 'select'
	});
	return true;
}

function deleteToLineStart(view: EditorView): boolean {
	const { state } = view;
	const range = state.selection.main;
	if (state.selection.ranges.length > 1 || !range.empty) return false;
	const line = state.doc.lineAt(range.head);
	if (inCode(state, line.from)) return false;
	const start = line.from + prefixOf(line.text).whole.length;
	const row = view.moveToLineBoundary(EditorSelection.cursor(range.head), false, true).head;
	if (row === line.from || row > start) return false;
	view.dispatch({
		changes: { from: line.from, to: range.head },
		selection: { anchor: line.from },
		scrollIntoView: true,
		userEvent: 'delete.backward'
	});
	return true;
}

export function softBreak(view: EditorView): boolean {
	const { state } = view;
	if (state.selection.ranges.some((range) => inCode(state, range.from))) return false;
	const tr = state.changeByRange((range) => {
		const { head, marker, gap, box } = prefixOf(state.doc.lineAt(range.from).text);
		const width = marker ? marker.length + gap.length + (box?.length ?? 0) : 0;
		const insert = `\n${head}${' '.repeat(width)}`;
		return {
			changes: { from: range.from, to: range.to, insert },
			range: EditorSelection.cursor(range.from + insert.length)
		};
	});
	view.dispatch(tr, { scrollIntoView: true, userEvent: 'input' });
	return true;
}

export function toggleChecklist(view: EditorView): boolean {
	const { state } = view;
	const changes: ChangeSpec[] = [];
	const seen = new Set<number>();
	for (const range of state.selection.ranges) {
		for (let pos = range.from; pos <= range.to; ) {
			const line = state.doc.lineAt(pos);
			pos = line.to + 1;
			if (seen.has(line.number)) continue;
			seen.add(line.number);
			const { head, marker, gap, box } = prefixOf(line.text);
			const afterMarker = line.from + head.length + (marker?.length ?? 0) + (gap?.length ?? 0);
			if (box) {
				const checked = box[1] !== ' ';
				changes.push({ from: afterMarker + 1, to: afterMarker + 2, insert: checked ? ' ' : 'x' });
			} else if (marker) changes.push({ from: afterMarker, insert: '[ ] ' });
			else changes.push({ from: line.from + head.length, insert: '- [ ] ' });
		}
	}
	const set = state.changes(changes);
	view.dispatch({ changes: set, selection: state.selection.map(set, 1), userEvent: 'input' });
	return true;
}

export const lineKeymap: readonly KeyBinding[] = [
	{
		key: 'Home',
		run: (view) => toContentStart(view, false),
		shift: (view) => toContentStart(view, true)
	},
	{
		mac: 'Mod-ArrowLeft',
		run: (view) => toContentStart(view, false),
		shift: (view) => toContentStart(view, true)
	},
	{ mac: 'Mod-Backspace', run: deleteToLineStart },
	{ key: 'Shift-Enter', run: softBreak },
	{ key: 'Mod-l', run: toggleChecklist, preventDefault: true }
];
