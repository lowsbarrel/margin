import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Line } from '@codemirror/state';
import { nodesNamed } from './doc-index';

export type Align = 'left' | 'center' | 'right' | 'none';

const HEADING_ABOVE = /^(?:#{1,6}(?:\s|$)|=+$|-+$)/;

export interface TableCell {
	text: string;
	from: number;
	to: number;
	slotFrom: number;
	slotTo: number;
}

interface TableRow {
	from: number;
	to: number;
	cells: TableCell[];
}

export interface TableModel {
	from: number;
	to: number;
	cols: number;
	align: Align[];
	rows: TableRow[];
}

function alignOf(text: string): Align {
	const value = text.trim();
	if (!/^:?-+:?$/.test(value)) return 'none';
	if (value.startsWith(':') && value.endsWith(':')) return 'center';
	if (value.endsWith(':')) return 'right';
	if (value.startsWith(':')) return 'left';
	return 'none';
}

// GFM escapes a literal pipe with a backslash, even inside a code span.
export function displayText(text: string): string {
	return text.replace(/\\([\\|])/g, '$1');
}

export function rowTexts(table: TableModel): string[][] {
	return table.rows.map((row) => row.cells.map((cell) => cell.text));
}

export function emptyRow(cols: number): string[] {
	return Array.from({ length: cols }, () => '');
}

// GFM splits a row on every pipe that is not backslash-escaped, code spans included.
function slotsOf(text: string): [number, number][] {
	const pipes: number[] = [];
	for (let at = 0; at < text.length; at++) {
		if (text[at] === '\\') at++;
		else if (text[at] === '|') pipes.push(at);
	}
	const start = text.length - text.trimStart().length;
	const end = text.trimEnd().length;
	const bounds = [...pipes];
	if (pipes[0] !== start) bounds.unshift(start - 1);
	if (pipes.at(-1) !== end - 1 || pipes.length === 0) bounds.push(end);
	const slots: [number, number][] = [];
	for (let index = 0; index + 1 < bounds.length; index++)
		slots.push([bounds[index] + 1, bounds[index + 1]]);
	return slots;
}

function cellsOf(line: Line): TableCell[] {
	return slotsOf(line.text).map(([from, to]) => {
		const raw = line.text.slice(from, to);
		const lead = raw.length - raw.trimStart().length;
		const text = raw.trim();
		const start = text ? from + lead : Math.min(from + (raw.startsWith(' ') ? 1 : 0), to);
		return {
			text,
			from: line.from + start,
			to: line.from + start + text.length,
			slotFrom: line.from + from,
			slotTo: line.from + to
		};
	});
}

function readTable(state: EditorState, node: SyntaxNode): TableModel {
	const rows: TableRow[] = [];
	let align: Align[] = [];
	for (let child = node.firstChild; child; child = child.nextSibling) {
		if (child.name === 'TableDelimiter') {
			align = state.doc.sliceString(child.from, child.to).split('|').slice(1, -1).map(alignOf);
			continue;
		}
		if (child.name !== 'TableHeader' && child.name !== 'TableRow') continue;
		const line: Line = state.doc.lineAt(child.from);
		rows.push({ from: line.from, to: line.to, cells: cellsOf(line) });
	}

	const cols = Math.max(1, align.length, ...rows.map((row) => row.cells.length));
	while (align.length < cols) align.push('none');
	for (const row of rows) {
		row.cells.length = Math.min(row.cells.length, cols);
		while (row.cells.length < cols)
			row.cells.push({ text: '', from: row.to, to: row.to, slotFrom: row.to, slotTo: row.to });
	}

	const head = state.doc.lineAt(node.from);
	const tail = state.doc.lineAt(
		state.doc.sliceString(node.to - 1, node.to) === '\n' ? node.to - 1 : node.to
	);
	return { from: head.from, to: tail.to, cols, align, rows };
}

function afterBlankOrHeading(state: EditorState, node: SyntaxNode): boolean {
	if (node.parent?.name !== 'Document') return false;
	const line = state.doc.lineAt(node.from);
	if (line.number === 1) return true;
	const above = state.doc.line(line.number - 1).text.trim();
	return !above || HEADING_ABOVE.test(above);
}

export function readTables(state: EditorState): TableModel[] {
	return nodesNamed(state, 'Table')
		.filter((node) => afterBlankOrHeading(state, node))
		.map((node) => readTable(state, node));
}

export function tableAt(state: EditorState, pos: number): TableModel | null {
	for (const side of [1, -1] as const) {
		let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, side);
		while (node) {
			if (node.name === 'Table' && afterBlankOrHeading(state, node)) return readTable(state, node);
			node = node.parent;
		}
	}
	return null;
}

export function onDelimiterRow(state: EditorState, table: TableModel, pos: number): boolean {
	const line = state.doc.lineAt(table.rows[0].to + 1);
	return pos >= line.from && pos <= line.to;
}

export function locateCell(table: TableModel, pos: number): { row: number; col: number } | null {
	for (let row = 0; row < table.rows.length; row++) {
		const at = table.rows[row];
		if (pos < at.from || pos > at.to) continue;
		for (let col = 0; col < at.cells.length; col++) {
			if (pos <= at.cells[col].slotTo) return { row, col };
		}
		return { row, col: at.cells.length - 1 };
	}
	return null;
}
