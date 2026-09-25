import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Line } from '@codemirror/state';

export type Align = 'left' | 'center' | 'right' | 'none';

interface TableCell {
	text: string;
	from: number;
	to: number;
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

const ALIGN_MARK: Record<Align, string> = {
	left: ':---',
	center: ':---:',
	right: '---:',
	none: '---'
};

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

export function textRows(rows: string[][], align: Align[]): string[][] {
	return [rows[0] ?? [], align.map((value) => ALIGN_MARK[value]), ...rows.slice(1)];
}

export function serializeTable(rows: string[][], align: Align[]): string {
	return textRows(rows, align)
		.map((cells) => `| ${cells.join(' | ')} |`)
		.join('\n');
}

export function rowTexts(table: TableModel): string[][] {
	return table.rows.map((row) => row.cells.map((cell) => cell.text));
}

export function emptyRow(cols: number): string[] {
	return Array.from({ length: cols }, () => '');
}

function rowEnd(state: EditorState, row: TableRow): number {
	const text = state.doc.sliceString(row.from, row.to).trimEnd();
	return text.endsWith('|') ? row.to - 1 : row.to;
}

function cellsOf(state: EditorState, node: SyntaxNode): TableCell[] {
	const cells: TableCell[] = [];
	for (let child = node.firstChild; child; child = child.nextSibling) {
		if (child.name !== 'TableCell') continue;
		cells.push({
			text: state.doc.sliceString(child.from, child.to),
			from: child.from,
			to: child.to
		});
	}
	return cells;
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
		rows.push({ from: line.from, to: line.to, cells: cellsOf(state, child) });
	}

	const cols = Math.max(1, align.length, ...rows.map((row) => row.cells.length));
	while (align.length < cols) align.push('none');
	for (const row of rows) {
		const end = rowEnd(state, row);
		while (row.cells.length < cols) row.cells.push({ text: '', from: end, to: end });
	}

	const head = state.doc.lineAt(node.from);
	const tail = state.doc.lineAt(
		state.doc.sliceString(node.to - 1, node.to) === '\n' ? node.to - 1 : node.to
	);
	return { from: head.from, to: tail.to, cols, align, rows };
}

export function readTables(state: EditorState): TableModel[] {
	const tables: TableModel[] = [];
	syntaxTree(state).iterate({
		enter(ref) {
			if (ref.name !== 'Table') return;
			if (ref.node.parent?.name !== 'Document') return false;
			tables.push(readTable(state, ref.node));
			return false;
		}
	});
	return tables;
}

export function tableAt(state: EditorState, pos: number): TableModel | null {
	for (const side of [1, -1] as const) {
		let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, side);
		while (node) {
			if (node.name === 'Table' && node.parent?.name === 'Document') return readTable(state, node);
			node = node.parent;
		}
	}
	return null;
}

export function locateCell(table: TableModel, pos: number): { row: number; col: number } | null {
	for (let row = 0; row < table.rows.length; row++) {
		const at = table.rows[row];
		if (pos < at.from || pos > at.to) continue;
		for (let col = 0; col < at.cells.length; col++) {
			if (pos <= at.cells[col].to) return { row, col };
		}
		return { row, col: at.cells.length - 1 };
	}
	return null;
}
