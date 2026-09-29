import { ensureSyntaxTree, indentUnit, syntaxTree } from '@codemirror/language';
import {
	countColumn,
	EditorSelection,
	type ChangeSet,
	type ChangeSpec,
	type EditorState,
	type Text,
	type TransactionSpec
} from '@codemirror/state';
import type { SyntaxNode, Tree } from '@lezer/common';

const LIST = /^(?:BulletList|OrderedList)$/;
const MARKER = /^([ \t]*)([-*+]|(\d{1,9})([.)]))([ \t]*)/;
const INDENTED_ITEM = /^([ \t]+)(?:[-*+]|\d{1,9}[.)])(?:[ \t]|$)/m;
const LOOKAHEAD = 50_000;

export interface ListItem {
	from: number;
	to: number;
	mark: number;
	indent: string;
	level: number;
	parent: ListItem | null;
	children: ListItem[];
}

export interface ListBlock {
	from: number;
	to: number;
	roots: ListItem[];
	items: ListItem[];
}

interface Marker {
	indent: string;
	token: string;
	number: number | null;
	delim: string;
	space: string;
}

export interface Target {
	pos: number;
	lead: string;
	tail: string;
}

export function treeUpTo(state: EditorState, pos: number): Tree {
	const upto = Math.min(state.doc.length, pos + LOOKAHEAD);
	return ensureSyntaxTree(state, upto, 100) ?? syntaxTree(state);
}

export function lineSpan(doc: Text, from: number, to: number): { from: number; to: number } {
	const first = doc.lineAt(from);
	let last = doc.lineAt(to);
	if (to === last.from && last.number > first.number) last = doc.line(last.number - 1);
	return { from: first.from, to: last.to };
}

export function leadingSpace(text: string): string {
	return /^[ \t]*/.exec(text)?.[0] ?? '';
}

function markerOf(text: string): Marker | null {
	const match = MARKER.exec(text);
	if (!match) return null;
	return {
		indent: match[1],
		token: match[2],
		number: match[3] === undefined ? null : Number(match[3]),
		delim: match[4] ?? '',
		space: match[5]
	};
}

function collect(
	doc: Text,
	list: SyntaxNode,
	parent: ListItem | null,
	level: number,
	out: ListItem[]
): ListItem[] {
	const siblings: ListItem[] = [];
	for (let node = list.firstChild; node; node = node.nextSibling) {
		if (node.name !== 'ListItem') continue;
		const line = doc.lineAt(node.from);
		const indent = leadingSpace(line.text);
		if (line.from + indent.length !== node.from) continue;
		const item: ListItem = {
			...lineSpan(doc, node.from, node.to),
			mark: node.from,
			indent,
			level,
			parent,
			children: []
		};
		out.push(item);
		for (let child = node.firstChild; child; child = child.nextSibling)
			if (LIST.test(child.name)) item.children.push(...collect(doc, child, item, level + 1, out));
		siblings.push(item);
	}
	return siblings;
}

function topList(tree: Tree, pos: number): SyntaxNode | null {
	for (const side of [1, -1] as const) {
		for (let node: SyntaxNode | null = tree.resolveInner(pos, side); node; node = node.parent)
			if (LIST.test(node.name) && node.parent?.name === 'Document') return node;
	}
	return null;
}

export function listAt(state: EditorState, pos: number): ListBlock | null {
	const top = topList(treeUpTo(state, pos), pos);
	if (!top) return null;
	const items: ListItem[] = [];
	const roots = collect(state.doc, top, null, 1, items);
	return roots.length ? { ...lineSpan(state.doc, top.from, top.to), roots, items } : null;
}

export function itemAt(block: ListBlock, pos: number): ListItem | null {
	let found: ListItem | null = null;
	for (const item of block.items)
		if (item.from <= pos && pos <= item.to && (!found || item.level > found.level)) found = item;
	return found;
}

export function ancestorAt(item: ListItem, level: number): ListItem {
	let at = item;
	while (at.level > level && at.parent) at = at.parent;
	return at;
}

export function columns(state: EditorState, indent: string): number {
	return countColumn(indent, state.tabSize);
}

export function prefersTabs(state: EditorState, ...indents: string[]): boolean {
	return state.facet(indentUnit) === '\t' || indents.some((indent) => indent.includes('\t'));
}

export function detectIndentUnit(text: string): string {
	return INDENTED_ITEM.exec(text)?.[1].startsWith(' ') ? '  ' : '\t';
}

function indentText(state: EditorState, cols: number, tabs: boolean): string {
	if (!tabs) return ' '.repeat(cols);
	return '\t'.repeat(Math.floor(cols / state.tabSize)) + ' '.repeat(cols % state.tabSize);
}

export function childColumn(state: EditorState, item: ListItem, tabs: boolean): number {
	const own = columns(state, item.indent);
	if (tabs) return own + state.tabSize;
	const marker = markerOf(state.doc.lineAt(item.from).text);
	if (!marker) return own + 2;
	const space = marker.space.length === 0 || marker.space.length > 4 ? 1 : marker.space.length;
	return own + marker.token.length + space;
}

export function tokenFor(state: EditorState, ref: ListItem | null, item: ListItem): string | null {
	const own = markerOf(state.doc.lineAt(item.from).text);
	if (!own) return null;
	if (!ref) return own.number !== null && own.number !== 1 ? `1${own.delim}` : null;
	const theirs = markerOf(state.doc.lineAt(ref.from).text);
	return theirs && theirs.token !== own.token ? theirs.token : null;
}

interface Releveled {
	text: string;
	map: (offset: number) => number;
}

export function relevel(
	state: EditorState,
	item: ListItem,
	cols: number,
	tabs: boolean,
	token: string | null
): Releveled {
	const lines = state.doc.sliceString(item.from, item.to).split('\n');
	const marker = markerOf(lines[0]);
	const oldToken = marker?.token ?? '';
	const newToken = token ?? oldToken;
	const shift = cols - columns(state, item.indent);
	const widen = newToken.length - oldToken.length;
	const oldStarts: number[] = [];
	const newStarts: number[] = [];
	const oldPrefix: number[] = [];
	const newPrefix: number[] = [];
	const out: string[] = [];
	let oldAt = 0;
	let newAt = 0;
	lines.forEach((line, index) => {
		const space = leadingSpace(line);
		let next = line;
		let before = space.length;
		let after = space.length;
		if (index === 0) {
			const indent = indentText(state, cols, tabs);
			next = indent + newToken + line.slice(space.length + oldToken.length);
			before = space.length + oldToken.length;
			after = indent.length + newToken.length;
		} else if (space.length < line.length) {
			const indent = indentText(state, Math.max(0, columns(state, space) + shift + widen), tabs);
			next = indent + line.slice(space.length);
			after = indent.length;
		}
		oldStarts.push(oldAt);
		newStarts.push(newAt);
		oldPrefix.push(before);
		newPrefix.push(after);
		out.push(next);
		oldAt += line.length + 1;
		newAt += next.length + 1;
	});
	const map = (offset: number): number => {
		let index = oldStarts.length - 1;
		while (index > 0 && oldStarts[index] > offset) index--;
		const within = offset - oldStarts[index];
		const moved =
			within >= oldPrefix[index]
				? within - oldPrefix[index] + newPrefix[index]
				: Math.min(within, newPrefix[index]);
		return newStarts[index] + moved;
	};
	return { text: out.join('\n'), map };
}

function renumberList(state: EditorState, list: SyntaxNode, changes: ChangeSpec[]): void {
	const numbers: { from: number; to: number; value: number }[] = [];
	for (let node = list.firstChild; node; node = node.nextSibling) {
		if (node.name !== 'ListItem') continue;
		const digits = /^\d+/.exec(state.doc.sliceString(node.from, node.from + 10));
		if (list.name === 'OrderedList' && digits)
			numbers.push({ from: node.from, to: node.from + digits[0].length, value: Number(digits[0]) });
		for (let child = node.firstChild; child; child = child.nextSibling)
			if (LIST.test(child.name)) renumberList(state, child, changes);
	}
	if (numbers.length < 2) return;
	const lazy = numbers.length >= 3 && numbers.every((entry) => entry.value === numbers[0].value);
	if (lazy) return;
	const start = Math.min(...numbers.map((entry) => entry.value));
	numbers.forEach((entry, index) => {
		const want = start + index;
		if (entry.value !== want)
			changes.push({ from: entry.from, to: entry.to, insert: String(want) });
	});
}

function renumberAt(state: EditorState, positions: number[]): ChangeSpec[] {
	const changes: ChangeSpec[] = [];
	const done = new Set<number>();
	for (const pos of positions) {
		const top = topList(treeUpTo(state, pos), Math.min(pos, state.doc.length));
		if (!top || done.has(top.from)) continue;
		done.add(top.from);
		renumberList(state, top, changes);
	}
	return changes;
}

export function listEdit(
	state: EditorState,
	changes: ChangeSpec[],
	touched: (set: ChangeSet) => number[],
	selection: (set: ChangeSet) => EditorSelection
): TransactionSpec | null {
	const first = state.changes(changes);
	const next = state.update({ changes: first }).state;
	if (next.doc.eq(state.doc)) return null;
	const second = next.changes(renumberAt(next, touched(first)));
	return { changes: first.compose(second), selection: selection(first).map(second) };
}

function removal(doc: Text, from: number, to: number): [number, number] {
	const first = doc.lineAt(from);
	const last = doc.lineAt(to);
	const before = first.number > 1 ? doc.line(first.number - 1) : null;
	if (last.number === doc.lines) return [before ? before.to : from, to];
	const after = doc.line(last.number + 1);
	const blankAround = (!before || !before.text.trim()) && !after.text.trim();
	if (blankAround && after.number < doc.lines) return [from, doc.line(after.number + 1).from];
	return [from, after.from];
}

export function relocateItem(
	state: EditorState,
	item: ListItem,
	target: Target,
	cols: number,
	tabs: boolean,
	token: string | null
): TransactionSpec | null {
	const [cutFrom, cutTo] = removal(state.doc, item.from, item.to);
	if (target.pos > cutFrom && target.pos < cutTo) return null;
	const { text, map } = relevel(state, item, cols, tabs, token);
	const changes: ChangeSpec[] = [
		{ from: cutFrom, to: cutTo },
		{ from: target.pos, insert: target.lead + text + target.tail }
	];
	const start = (set: ChangeSet) => set.mapPos(target.pos, -1) + target.lead.length;
	const moveTo = (set: ChangeSet, pos: number) =>
		pos >= item.from && pos <= item.to ? start(set) + map(pos - item.from) : set.mapPos(pos);
	return listEdit(
		state,
		changes,
		(set) => [start(set), set.mapPos(cutFrom, 1)],
		(set) =>
			EditorSelection.create(
				state.selection.ranges.map((range) =>
					EditorSelection.range(moveTo(set, range.anchor), moveTo(set, range.head))
				),
				state.selection.mainIndex
			)
	);
}
