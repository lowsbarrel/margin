import type { EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { locateCell, onDelimiterRow, tableAt, type TableModel } from './table-model';
import { appendRow, deleteRow } from './table-ops';

export interface CellAt {
	table: TableModel;
	row: number;
	col: number;
}

export function cellAt(state: EditorState, pos: number): CellAt | null {
	const table = tableAt(state, pos);
	if (!table) return null;
	const cell = locateCell(table, pos);
	return cell ? { table, ...cell } : null;
}

export function locate(view: EditorView): CellAt | null {
	return cellAt(view.state, view.state.selection.main.head);
}

// A row shorter than the header has no text for its trailing cells, so they are written out before the caret goes there.
export function ensureCell(view: EditorView, at: CellAt): number {
	const row = at.table.rows[at.row];
	const text = view.state.doc.sliceString(row.from, row.to);
	const missingAt = row.cells.findIndex(
		(cell) => cell.slotFrom === row.to && cell.slotTo === row.to
	);
	const real = missingAt < 0 ? row.cells.length : missingAt;
	if (at.col < real) return row.cells[at.col].from;
	const closed = /\|\s*$/.test(text);
	const missing = at.col + 1 - real;
	const insert = `${closed ? '' : ' |'}${'  |'.repeat(missing)}`;
	const end = row.to - (text.length - text.trimEnd().length);
	view.dispatch({ changes: { from: end, to: row.to, insert }, userEvent: 'input' });
	return end + insert.length - 2;
}

function enterCellEnd(view: EditorView, table: TableModel, row: number, col: number): boolean {
	const cell = table.rows[row]?.cells[col];
	if (!cell) return false;
	const from = ensureCell(view, { table, row, col });
	const current = cellAt(view.state, from);
	const target = current?.table.rows[row]?.cells[col] ?? cell;
	view.dispatch({ selection: { anchor: target.to }, scrollIntoView: true });
	return true;
}

export function nextCell(view: EditorView): boolean {
	const at = locate(view);
	if (!at) return false;
	if (at.col + 1 < at.table.cols) return enterCellEnd(view, at.table, at.row, at.col + 1);
	if (at.row + 1 < at.table.rows.length) return enterCellEnd(view, at.table, at.row + 1, 0);
	appendRow(view, at.table, 0);
	return true;
}

export function previousCell(view: EditorView): boolean {
	const at = locate(view);
	if (!at) return false;
	if (at.col > 0) return enterCellEnd(view, at.table, at.row, at.col - 1);
	if (at.row > 0) return enterCellEnd(view, at.table, at.row - 1, at.table.cols - 1);
	return true;
}

export function enterCell(view: EditorView, col: number | null = null): boolean {
	const at = locate(view);
	if (!at) return false;
	const last = at.row + 1 === at.table.rows.length;
	if (last && at.row > 0 && at.table.rows[at.row].cells.every((cell) => !cell.text)) {
		deleteRow(view, at.table, at.row);
		return leaveTable(view);
	}
	const target = col ?? at.col;
	if (!last) return enterCellEnd(view, at.table, at.row + 1, target);
	appendRow(view, at.table, target);
	return true;
}

export function leaveDelimiter(view: EditorView): boolean {
	const { head } = view.state.selection.main;
	const table = tableAt(view.state, head);
	if (!table || !onDelimiterRow(view.state, table, head)) return false;
	if (table.rows.length > 1) return enterCellEnd(view, table, 1, 0);
	appendRow(view, table, 0);
	return true;
}

export function leaveTable(view: EditorView, dir: -1 | 1 = 1): boolean {
	const at = locate(view);
	if (!at) return false;
	const doc = view.state.doc;
	if (dir < 0) {
		const first = doc.lineAt(at.table.from);
		if (first.number === 1) {
			view.dispatch({ changes: { from: 0, insert: '\n' }, selection: { anchor: 0 } });
		} else
			view.dispatch({ selection: { anchor: doc.line(first.number - 1).to }, scrollIntoView: true });
	} else {
		const last = doc.lineAt(at.table.to);
		if (last.number === doc.lines) {
			view.dispatch({
				changes: { from: last.to, insert: '\n' },
				selection: { anchor: last.to + 1 }
			});
		} else
			view.dispatch({
				selection: { anchor: doc.line(last.number + 1).from },
				scrollIntoView: true
			});
	}
	view.focus();
	return true;
}

export function verticalCell(view: EditorView, dir: -1 | 1): boolean {
	const at = locate(view);
	if (!at) return false;
	const row = at.row + dir;
	if (row < 0 || row >= at.table.rows.length) return leaveTable(view, dir);
	const from = ensureCell(view, { table: at.table, row, col: at.col });
	view.dispatch({ selection: { anchor: from }, scrollIntoView: true });
	return true;
}

export function horizontalCell(view: EditorView, dir: -1 | 1): boolean {
	const at = locate(view);
	if (!at) return false;
	let { row, col } = at;
	col += dir;
	if (col < 0 || col >= at.table.cols) {
		row += dir;
		col = dir > 0 ? 0 : at.table.cols - 1;
	}
	if (row < 0 || row >= at.table.rows.length) return leaveTable(view, dir);
	const from = ensureCell(view, { table: at.table, row, col });
	const target = cellAt(view.state, from)?.table.rows[row].cells[col];
	const anchor = dir > 0 ? from : (target?.to ?? from);
	view.dispatch({ selection: { anchor }, scrollIntoView: true });
	return true;
}

// Arrow keys at a paragraph edge step into the neighbouring table instead of jumping over its widget.
export function enterTable(view: EditorView, dir: -1 | 1): boolean {
	const range = view.state.selection.main;
	if (!range.empty) return false;
	const doc = view.state.doc;
	const line = doc.lineAt(range.head);
	const number = line.number + dir;
	if (number < 1 || number > doc.lines) return false;
	const at = cellAt(view.state, doc.line(number).from);
	if (!at || locate(view)) return false;
	const row = dir > 0 ? 0 : at.table.rows.length - 1;
	const from = ensureCell(view, { table: at.table, row, col: 0 });
	view.dispatch({ selection: { anchor: from }, scrollIntoView: true });
	return true;
}
