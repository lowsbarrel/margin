import { syntaxTree } from '@codemirror/language';
import { EditorSelection, type EditorState, type SelectionRange } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { SyntaxNode, SyntaxNodeRef } from '@lezer/common';
import { COMMENT, HIGHLIGHT } from './syntax';

export type MarkKind = 'bold' | 'italic' | 'strike' | 'code' | 'highlight' | 'comment';

export const MARKER: Record<MarkKind, string> = {
	bold: '**',
	italic: '*',
	strike: '~~',
	code: '`',
	highlight: '==',
	comment: '%%'
};

const KIND_BY_NODE: Record<string, MarkKind> = {
	StrongEmphasis: 'bold',
	Emphasis: 'italic',
	Strikethrough: 'strike',
	InlineCode: 'code',
	[HIGHLIGHT]: 'highlight',
	[COMMENT]: 'comment'
};

interface Range {
	from: number;
	to: number;
}

interface Span extends Range {
	kind: MarkKind;
	open: Range;
	close: Range;
}

function covers(outer: Range, inner: Range): boolean {
	return outer.from <= inner.from && outer.to >= inner.to;
}

// Markdown's own delimiters differ per mark and per written form (`**` vs `__`), so they come from the tree.
function delimiters(
	state: EditorState,
	kind: MarkKind,
	node: SyntaxNode
): { open: Range; close: Range } | null {
	if (kind === 'code' || kind === 'comment') {
		const text = state.sliceDoc(node.from, node.to);
		const open = (kind === 'code' ? /^`+/ : /^%%/).exec(text)?.[0].length ?? 0;
		const close = (kind === 'code' ? /`+$/ : /%%$/).exec(text)?.[0].length ?? 0;
		if (node.to - node.from <= open + close) return null;
		return {
			open: { from: node.from, to: node.from + open },
			close: { from: node.to - close, to: node.to }
		};
	}
	const first = node.firstChild;
	const last = node.lastChild;
	if (!first || !last || (first.from === last.from && first.to === last.to)) return null;
	if (!first.name.endsWith('Mark') || !last.name.endsWith('Mark')) return null;
	return { open: { from: first.from, to: first.to }, close: { from: last.from, to: last.to } };
}

function markSpans(state: EditorState, from: number, to: number): Span[] {
	const spans: Span[] = [];
	syntaxTree(state).iterate({
		from,
		to,
		enter: (ref: SyntaxNodeRef) => {
			const kind = KIND_BY_NODE[ref.name];
			if (!kind) return;
			const marks = delimiters(state, kind, ref.node);
			if (marks) spans.push({ kind, from: ref.from, to: ref.to, ...marks });
		}
	});
	return spans;
}

export function activeMarks(state: EditorState): MarkKind[] {
	const range = state.selection.main;
	if (range.empty) return [];
	const kinds = new Set<MarkKind>();
	for (const span of markSpans(state, range.from, range.to)) {
		if (covers(span, range) || covers(range, span)) kinds.add(span.kind);
	}
	return [...kinds];
}

function contentRange(range: Range, spans: Span[]): Range {
	const ordered = [...spans].sort((a, b) => b.to - b.from - (a.to - a.from));
	let inner = range;
	for (const span of ordered) {
		if (covers(span, inner)) inner = { from: span.open.to, to: span.close.from };
	}
	return inner;
}

function addMark(
	state: EditorState,
	kind: MarkKind,
	target: Range,
	spans: Span[],
	original: SelectionRange
) {
	const inner = contentRange(target, spans);
	const marker = MARKER[kind];
	const changes = state.changes([
		{ from: inner.from, insert: marker },
		{ from: inner.to, insert: marker }
	]);
	const map = (pos: number) => changes.mapPos(pos, pos === original.from ? 1 : -1);
	const range =
		original.empty && kind !== 'comment'
			? EditorSelection.cursor(changes.mapPos(original.head, 1))
			: original.empty
				? EditorSelection.range(changes.mapPos(inner.from, 1), changes.mapPos(inner.to, -1))
				: EditorSelection.range(map(original.anchor), map(original.head));
	return { changes, range };
}

function markRange(state: EditorState, kind: MarkKind, range: SelectionRange) {
	const spans = markSpans(state, range.from, range.to);
	const active = spans.find(
		(span) =>
			span.kind === kind &&
			(range.empty
				? span.open.to <= range.from && range.from <= span.close.from
				: covers(span, range) || covers(range, span))
	);
	if (active) {
		const changes = state.changes([
			{ from: active.open.from, to: active.open.to },
			{ from: active.close.from, to: active.close.to }
		]);
		return { changes, range: range.map(changes) };
	}
	const target = range.empty ? state.wordAt(range.head) : range;
	if (target) return addMark(state, kind, target, spans, range);
	const marker = MARKER[kind];
	return {
		changes: { from: range.from, insert: marker + marker },
		range: EditorSelection.cursor(range.from + marker.length)
	};
}

export function toggleMark(view: EditorView, kind: MarkKind): boolean {
	const { state } = view;
	view.dispatch(
		state.changeByRange((range) => markRange(state, kind, range)),
		{ userEvent: 'input' }
	);
	return true;
}
