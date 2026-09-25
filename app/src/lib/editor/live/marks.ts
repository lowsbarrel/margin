import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import type { SyntaxNode, SyntaxNodeRef } from '@lezer/common';
import { HIGHLIGHT } from './syntax';

export type MarkKind = 'bold' | 'italic' | 'strike' | 'code' | 'highlight';

const MARKER: Record<MarkKind, string> = {
	bold: '**',
	italic: '*',
	strike: '~~',
	code: '`',
	highlight: '=='
};

const KIND_BY_NODE: Record<string, MarkKind> = {
	StrongEmphasis: 'bold',
	Emphasis: 'italic',
	Strikethrough: 'strike',
	InlineCode: 'code',
	[HIGHLIGHT]: 'highlight'
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
	if (kind === 'code') {
		const text = state.sliceDoc(node.from, node.to);
		const open = /^`+/.exec(text)?.[0].length ?? 0;
		const close = /`+$/.exec(text)?.[0].length ?? 0;
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

function insertMark(view: EditorView, kind: MarkKind, at: number): boolean {
	const marker = MARKER[kind];
	view.dispatch({
		changes: { from: at, insert: marker + marker },
		selection: { anchor: at + marker.length },
		userEvent: 'input'
	});
	return true;
}

function removeMark(view: EditorView, span: Span): boolean {
	view.dispatch({
		changes: [
			{ from: span.open.from, to: span.open.to },
			{ from: span.close.from, to: span.close.to }
		],
		selection: { anchor: span.open.from, head: span.open.from + (span.close.from - span.open.to) },
		userEvent: 'input'
	});
	return true;
}

function addMark(view: EditorView, kind: MarkKind, range: Range, spans: Span[]): boolean {
	const inner = contentRange(range, spans);
	const marker = MARKER[kind];
	view.dispatch({
		changes: [
			{ from: inner.from, insert: marker },
			{ from: inner.to, insert: marker }
		],
		selection: { anchor: inner.from, head: inner.to + marker.length * 2 },
		userEvent: 'input'
	});
	return true;
}

export function toggleMark(view: EditorView, kind: MarkKind): boolean {
	const { state } = view;
	const range = state.selection.main;
	if (range.empty) return insertMark(view, kind, range.from);
	const spans = markSpans(state, range.from, range.to);
	const active = spans.find(
		(span) => span.kind === kind && (covers(span, range) || covers(range, span))
	);
	if (active) return removeMark(view, active);
	return addMark(view, kind, range, spans);
}
