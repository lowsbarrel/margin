import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
import { syntaxTree } from '@codemirror/language';
import {
	EditorSelection,
	Prec,
	type EditorState,
	type Extension,
	type SelectionRange
} from '@codemirror/state';
import { EditorView, keymap, type Command } from '@codemirror/view';
import type { SyntaxNode } from '@lezer/common';

const WRAPS: Record<string, true> = {
	'*': true,
	_: true,
	'`': true,
	'=': true,
	'~': true,
	$: true,
	'%': true
};
const PAIRS: Record<string, true> = { '*': true, _: true, '`': true };
const ACROSS_LINES: Record<string, string> = {
	'*': '*',
	_: '_',
	'(': ')',
	'[': ']',
	'{': '}',
	'"': '"',
	"'": "'"
};
const CODE_BLOCKS: Record<string, true> = { FencedCode: true, CodeBlock: true };

interface Around {
	lead: number;
	trail: number;
	prev: string;
	next: string;
}

function inCodeBlock(state: EditorState, pos: number): boolean {
	let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, -1);
	while (node) {
		if (CODE_BLOCKS[node.name]) return state.doc.lineAt(node.from).to < pos;
		node = node.parent;
	}
	return false;
}

function runLength(text: string, mark: string, backward: boolean): number {
	let count = 0;
	while (count < text.length && text[backward ? text.length - 1 - count : count] === mark) count++;
	return count;
}

function around(state: EditorState, pos: number, mark: string): Around {
	const line = state.doc.lineAt(pos);
	const before = state.sliceDoc(line.from, pos);
	const after = state.sliceDoc(pos, line.to);
	const lead = runLength(before, mark, true);
	const trail = runLength(after, mark, false);
	return { lead, trail, prev: before[before.length - lead - 1] ?? '', next: after[trail] ?? '' };
}

function boundary(char: string): boolean {
	return char === '' || /\s/.test(char);
}

function emptyPair(at: Around): boolean {
	return at.lead === at.trail && boundary(at.prev) && boundary(at.next);
}

function wrapRange(state: EditorState, range: SelectionRange, mark: string) {
	const fence =
		mark === '`' &&
		state.sliceDoc(range.from - 2, range.from) === '``' &&
		state.sliceDoc(range.to, range.to + 2) === '``';
	if (!fence)
		return {
			changes: [
				{ from: range.from, insert: mark },
				{ from: range.to, insert: mark }
			],
			range: EditorSelection.range(range.anchor + 1, range.head + 1)
		};
	const from = range.from - 2;
	const to = range.to + 2;
	const open = from === state.doc.lineAt(from).from ? '```\n' : '\n```\n';
	const close = to === state.doc.lineAt(to).to ? '\n```' : '\n```\n';
	const body = state.sliceDoc(range.from, range.to);
	return {
		changes: { from, to, insert: open + body + close },
		range: EditorSelection.range(from + open.length, from + open.length + body.length)
	};
}

function pairRange(state: EditorState, range: SelectionRange, mark: string) {
	const pos = range.head;
	const at = around(state, pos, mark);
	if (emptyPair(at)) {
		const lineFrom = state.doc.lineAt(pos).from;
		const indent = state.sliceDoc(lineFrom, Math.max(lineFrom, pos - 2));
		if (mark === '`' && at.lead === 2 && !indent.trim())
			return {
				changes: { from: pos - 2, to: pos + 2, insert: `\`\`\`\n${indent}\`\`\`` },
				range: EditorSelection.cursor(pos + 1)
			};
		return { changes: { from: pos, insert: mark + mark }, range: EditorSelection.cursor(pos + 1) };
	}
	if (at.trail > 0 && !/\w/.test(at.next)) return { range: EditorSelection.cursor(pos + 1) };
	return null;
}

function typeMark(view: EditorView, mark: string): boolean {
	const { state } = view;
	if (inCodeBlock(state, state.selection.main.head)) return false;
	const wrapping = state.selection.ranges.some((range) => !range.empty);
	if (!wrapping && !PAIRS[mark]) return false;
	let handled = false;
	const tr = state.changeByRange((range) => {
		const spec = wrapping
			? range.empty
				? null
				: wrapRange(state, range, mark)
			: pairRange(state, range, mark);
		if (spec) {
			handled = true;
			return spec;
		}
		return {
			changes: { from: range.from, to: range.to, insert: mark },
			range: EditorSelection.cursor(range.from + mark.length)
		};
	});
	if (!handled) return false;
	view.dispatch(tr, { userEvent: 'input.type', scrollIntoView: true });
	return true;
}

function spansLines(state: EditorState, range: SelectionRange): boolean {
	return state.doc.lineAt(range.from).number !== state.doc.lineAt(range.to).number;
}

// Typed over a selection that crosses lines, a pairing character replaces it instead of wrapping it.
function pairAcrossLines(view: EditorView, open: string): boolean {
	const { state } = view;
	const close = ACROSS_LINES[open];
	if (!close || !state.selection.ranges.some((range) => spansLines(state, range))) return false;
	const tr = state.changeByRange((range) =>
		spansLines(state, range)
			? {
					changes: { from: range.from, to: range.to, insert: open + close },
					range: EditorSelection.cursor(range.from + open.length)
				}
			: {
					changes: [
						{ from: range.from, insert: open },
						{ from: range.to, insert: close }
					],
					range: EditorSelection.range(range.anchor + open.length, range.head + open.length)
				}
	);
	view.dispatch(tr, { userEvent: 'input.type', scrollIntoView: true });
	return true;
}

// Obsidian 1.5 stopped pairing the apostrophe typed after a link, as in `[[link]]'s`.
function plainApostrophe(view: EditorView): boolean {
	const { state } = view;
	const { head, empty } = state.selection.main;
	if (!empty || state.sliceDoc(head - 1, head) !== ']') return false;
	view.dispatch(state.replaceSelection("'"), { userEvent: 'input.type' });
	return true;
}

function singleEmptyPair(state: EditorState): string | null {
	const range = state.selection.main;
	if (state.selection.ranges.length > 1 || !range.empty) return null;
	const mark = state.sliceDoc(range.head - 1, range.head);
	if (!PAIRS[mark] || state.sliceDoc(range.head, range.head + 1) !== mark) return null;
	return emptyPair(around(state, range.head, mark)) ? mark : null;
}

function unpairOnSpace(view: EditorView): boolean {
	const { state } = view;
	const mark = singleEmptyPair(state);
	const head = state.selection.main.head;
	if (!mark || around(state, head, mark).lead !== 1) return false;
	view.dispatch({
		changes: { from: head, to: head + 1, insert: ' ' },
		selection: { anchor: head + 1 },
		userEvent: 'input.type'
	});
	return true;
}

const deleteMarkPair: Command = (view) => {
	const head = view.state.selection.main.head;
	if (!singleEmptyPair(view.state)) return false;
	view.dispatch({
		changes: { from: head - 1, to: head + 1 },
		selection: { anchor: head - 1 },
		userEvent: 'delete.backward'
	});
	return true;
};

export const autoPair: Extension = [
	Prec.high(
		EditorView.inputHandler.of((view, _from, _to, text) => {
			if (view.composing || view.state.readOnly) return false;
			if (pairAcrossLines(view, text)) return true;
			if (text === "'") return plainApostrophe(view);
			if (text === ' ') return unpairOnSpace(view);
			return WRAPS[text] ? typeMark(view, text) : false;
		})
	),
	closeBrackets(),
	Prec.high(keymap.of([{ key: 'Backspace', run: deleteMarkPair }, ...closeBracketsKeymap]))
];
