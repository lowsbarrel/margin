import { syntaxTree } from '@codemirror/language';
import type { EditorSelection, EditorState, Line, Range, Text } from '@codemirror/state';
import { StateEffect } from '@codemirror/state';
import { Decoration, type ViewUpdate } from '@codemirror/view';
import { staticPreview, type LiveContext } from './context';
import { noteDir, type ResolveSources } from './resolve';
import { vaultIndexReady } from './vault-index';

export const refreshDecorations = StateEffect.define<null>();

// The background parser extends the tree without touching the document, and decorations have to follow it.
export function treeChanged(update: ViewUpdate): boolean {
	return syntaxTree(update.state) !== syntaxTree(update.startState);
}

export function sourcesOf(ctx: LiveContext): ResolveSources {
	return {
		vaultPath: () => ctx.vaultPath(),
		noteDir: () => noteDir(ctx.notePath(), ctx.vaultPath()),
		attachmentFolder: () => ctx.attachmentFolder(),
		exists: (relPath) => ctx.exists(relPath),
		findByName: (name) => ctx.findByName(name),
		hasIndex: () => vaultIndexReady()
	};
}

export function hide(from: number, to: number): Range<Decoration> {
	return Decoration.replace({}).range(from, to);
}

export function mark(
	from: number,
	to: number,
	className: string,
	attributes?: Record<string, string>
): Range<Decoration> {
	return Decoration.mark({ class: className, attributes }).range(from, to);
}

export function line(from: number, className: string): Range<Decoration> {
	return Decoration.line({ class: className }).range(from);
}

function selectedLines(doc: Text, selection: EditorSelection): Set<number> {
	const lines = new Set<number>();
	for (const range of selection.ranges) {
		const first = doc.lineAt(range.from).number;
		const last = doc.lineAt(range.to).number;
		for (let number = first; number <= last; number++) lines.add(number);
	}
	return lines;
}

export function touchedLines(state: EditorState): Set<number> {
	if (state.facet(staticPreview)) return new Set();
	return selectedLines(state.doc, state.selection);
}

export function revealMoved(start: EditorState, doc: Text, selection: EditorSelection): boolean {
	if (start.facet(staticPreview) || start.selection.eq(selection)) return false;
	const before = selectedLines(start.doc, start.selection);
	const after = selectedLines(doc, selection);
	if (before.size !== after.size) return true;
	for (const number of before) if (!after.has(number)) return true;
	return false;
}

export function frontmatterEnd(state: EditorState): number {
	const doc = state.doc;
	if (!/^---[ \t]*\r?\n/.test(doc.sliceString(0, Math.min(doc.length, 512)))) return 0;
	for (let number = 2; number <= doc.lines; number++) {
		const text = doc.line(number).text;
		if (/^(---|\.\.\.)[ \t]*$/.test(text)) return doc.line(number).to;
	}
	return 0;
}

export function touched(
	state: EditorState,
	touchedSet: Set<number>,
	from: number,
	to: number
): boolean {
	const doc = state.doc;
	const first = doc.lineAt(Math.max(0, Math.min(from, doc.length))).number;
	const last = doc.lineAt(Math.max(0, Math.min(to, doc.length))).number;
	for (let number = first; number <= last; number++) {
		if (touchedSet.has(number)) return true;
	}
	return false;
}

export function eachLine(
	state: EditorState,
	from: number,
	to: number,
	fn: (line: Line) => void
): void {
	const doc = state.doc;
	const start = doc.lineAt(Math.max(0, Math.min(from, doc.length)));
	const end = doc.lineAt(Math.max(start.from, Math.min(Math.max(to - 1, from), doc.length)));
	for (let number = start.number; number <= end.number; number++) fn(doc.line(number));
}

// A hidden multi-line block keeps its line boxes, so every line in the range has to collapse.
export function collapsedLines(state: EditorState, from: number, to: number): Range<Decoration>[] {
	const ranges: Range<Decoration>[] = [];
	eachLine(state, from, to, (at) => ranges.push(line(at.from, 'cm-lp-block-line')));
	return ranges;
}
