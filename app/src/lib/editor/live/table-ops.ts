import type { TransactionSpec } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import {
	emptyRow,
	rowTexts,
	serializeTable,
	textRows,
	type Align,
	type TableModel
} from './table-model';

interface Rewrite {
	rows: string[][];
	align: Align[];
	select?: { row: number; col: number };
}

function rowLength(cells: string[]): number {
	return cells.join(' | ').length + 4;
}

// Model rows skip the delimiter line the serializer writes after the header.
function cellOffset(lines: string[][], select: { row: number; col: number }): number {
	const target = select.row === 0 ? 0 : select.row + 1;
	let at = 2;
	for (let index = 0; index < target; index++) at += rowLength(lines[index]) + 1;
	for (let index = 0; index < select.col; index++) at += (lines[target]?.[index] ?? '').length + 3;
	return at;
}

function applied(view: EditorView, table: TableModel, next: Rewrite): void {
	const text = serializeTable(next.rows, next.align);
	const spec: TransactionSpec = {
		changes: { from: table.from, to: table.to, insert: text },
		scrollIntoView: true,
		userEvent: 'input'
	};
	if (next.select) {
		const lines = textRows(next.rows, next.align);
		const from = table.from + cellOffset(lines, next.select);
		spec.selection = {
			anchor: from,
			head:
				from +
				(lines[next.select.row === 0 ? 0 : next.select.row + 1]?.[next.select.col] ?? '').length
		};
	}
	view.dispatch(spec);
}

function paddedRows(table: TableModel): string[][] {
	return rowTexts(table).map((cells) => {
		const row = cells.slice(0, table.cols);
		while (row.length < table.cols) row.push('');
		return row;
	});
}

export function insertRow(
	view: EditorView,
	table: TableModel,
	index: number,
	side: 'above' | 'below'
): void {
	const rows = paddedRows(table);
	rows.splice(side === 'above' ? index : index + 1, 0, emptyRow(table.cols));
	applied(view, table, { rows, align: table.align.slice() });
}

export function deleteRow(view: EditorView, table: TableModel, index: number): void {
	if (table.rows.length <= 1) return;
	const rows = paddedRows(table);
	rows.splice(index, 1);
	applied(view, table, { rows, align: table.align.slice() });
}

export function moveRowTo(view: EditorView, table: TableModel, from: number, to: number): void {
	if (from === to || to < 0 || from < 0 || from >= table.rows.length || to >= table.rows.length)
		return;
	const rows = paddedRows(table);
	const [moved] = rows.splice(from, 1);
	rows.splice(to, 0, moved);
	applied(view, table, { rows, align: table.align.slice() });
}

export function appendRow(
	view: EditorView,
	table: TableModel,
	selectCol: number | null = null
): void {
	const rows = paddedRows(table);
	rows.push(emptyRow(table.cols));
	applied(view, table, {
		rows,
		align: table.align.slice(),
		select: selectCol === null ? undefined : { row: rows.length - 1, col: selectCol }
	});
}

export function insertColumn(
	view: EditorView,
	table: TableModel,
	index: number,
	side: 'left' | 'right'
): void {
	const at = side === 'left' ? index : index + 1;
	const rows = paddedRows(table).map((cells) => {
		cells.splice(at, 0, '');
		return cells;
	});
	const align = table.align.slice();
	align.splice(at, 0, 'none');
	applied(view, table, { rows, align });
}

export function deleteColumn(view: EditorView, table: TableModel, index: number): void {
	if (table.cols <= 1) return;
	const rows = paddedRows(table).map((cells) => {
		cells.splice(index, 1);
		return cells;
	});
	const align = table.align.slice();
	align.splice(index, 1);
	applied(view, table, { rows, align });
}

export function moveColumnTo(view: EditorView, table: TableModel, from: number, to: number): void {
	if (from === to || to < 0 || from < 0 || from >= table.cols || to >= table.cols) return;
	const rows = paddedRows(table).map((cells) => {
		const [moved] = cells.splice(from, 1);
		cells.splice(to, 0, moved);
		return cells;
	});
	const align = table.align.slice();
	const [moved] = align.splice(from, 1);
	align.splice(to, 0, moved);
	applied(view, table, { rows, align });
}

export function alignColumn(
	view: EditorView,
	table: TableModel,
	index: number,
	align: Align
): void {
	const next = table.align.slice();
	next[index] = align;
	applied(view, table, { rows: paddedRows(table), align: next });
}
