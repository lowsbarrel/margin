import type { TransactionSpec } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { tableEdit, type CellPlace } from './table-format';
import { emptyRow, rowTexts, type Align, type TableModel } from './table-model';

export interface Rewrite {
	rows: string[][];
	align: Align[];
	select?: CellPlace;
}

export function writeTable(
	view: EditorView,
	table: TableModel,
	next: Rewrite,
	extra: TransactionSpec = {}
): void {
	const edit = tableEdit(view.state.doc, table, next.rows, next.align);
	const spec: TransactionSpec = {
		changes: edit.changes,
		scrollIntoView: true,
		userEvent: 'input',
		...extra
	};
	if (next.select) {
		const text = next.rows[next.select.row]?.[next.select.col] ?? '';
		spec.selection = { anchor: edit.cellStart(next.select) + text.length };
	}
	view.dispatch(spec);
}

export function rowsOf(table: TableModel): string[][] {
	return rowTexts(table).map((cells) => cells.slice());
}

export function insertRow(
	view: EditorView,
	table: TableModel,
	index: number,
	side: 'above' | 'below'
): void {
	const rows = rowsOf(table);
	rows.splice(side === 'above' ? index : index + 1, 0, emptyRow(table.cols));
	writeTable(view, table, { rows, align: table.align.slice() });
}

export function deleteRow(view: EditorView, table: TableModel, index: number): void {
	if (table.rows.length <= 1) return;
	const rows = rowsOf(table);
	rows.splice(index, 1);
	writeTable(view, table, { rows, align: table.align.slice() });
}

export function moveRowTo(view: EditorView, table: TableModel, from: number, to: number): void {
	if (from === to || to < 0 || from < 0 || from >= table.rows.length || to >= table.rows.length)
		return;
	const rows = rowsOf(table);
	const [moved] = rows.splice(from, 1);
	rows.splice(to, 0, moved);
	writeTable(view, table, { rows, align: table.align.slice() });
}

export function appendRow(
	view: EditorView,
	table: TableModel,
	selectCol: number | null = null
): void {
	const rows = rowsOf(table);
	rows.push(emptyRow(table.cols));
	writeTable(view, table, {
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
	const rows = rowsOf(table).map((cells) => {
		cells.splice(at, 0, '');
		return cells;
	});
	const align = table.align.slice();
	align.splice(at, 0, 'none');
	writeTable(view, table, { rows, align });
}

export function deleteColumn(view: EditorView, table: TableModel, index: number): void {
	if (table.cols <= 1) return;
	const rows = rowsOf(table).map((cells) => {
		cells.splice(index, 1);
		return cells;
	});
	const align = table.align.slice();
	align.splice(index, 1);
	writeTable(view, table, { rows, align });
}

export function moveColumnTo(view: EditorView, table: TableModel, from: number, to: number): void {
	if (from === to || to < 0 || from < 0 || from >= table.cols || to >= table.cols) return;
	const rows = rowsOf(table).map((cells) => {
		const [moved] = cells.splice(from, 1);
		cells.splice(to, 0, moved);
		return cells;
	});
	const align = table.align.slice();
	const [moved] = align.splice(from, 1);
	align.splice(to, 0, moved);
	writeTable(view, table, { rows, align });
}

export function alignColumns(
	view: EditorView,
	table: TableModel,
	cols: readonly number[],
	align: Align
): void {
	const next = table.align.slice();
	for (const col of cols) next[col] = align;
	writeTable(view, table, { rows: rowsOf(table), align: next });
}

function sortKey(text: string): string {
	return text
		.replace(
			/!?\[\[([^\]|]*)(?:\\?\|([^\]]*))?\]\]/g,
			(_, target: string, alias?: string) => alias ?? target
		)
		.replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
		.replace(/[*_~=`\\]/g, '')
		.trim();
}

export function sortRows(view: EditorView, table: TableModel, col: number, dir: 1 | -1): void {
	const [head, ...body] = rowsOf(table);
	const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
	body.sort((a, b) => dir * collator.compare(sortKey(a[col] ?? ''), sortKey(b[col] ?? '')));
	writeTable(view, table, { rows: [head, ...body], align: table.align.slice() });
}
