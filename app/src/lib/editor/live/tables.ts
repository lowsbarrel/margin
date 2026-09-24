import type { EditorState, Extension, Range } from '@codemirror/state';
import { StateField } from '@codemirror/state';
import { Decoration, EditorView, ViewPlugin, keymap, type DecorationSet } from '@codemirror/view';
import { eachLine, line, refreshDecorations, touched, touchedLines } from './decorate';
import { ESCAPE_TABLE, onEscape } from './escape';
import { locateCell, readTables, tableAt, type TableModel } from './table-model';
import { appendRow } from './table-ops';
import { TableWidget } from './table-widget';
import './tables.css';

function tableDecorations(state: EditorState): DecorationSet {
	const touchedSet = touchedLines(state);
	const ranges: Range<Decoration>[] = [];
	for (const table of readTables(state)) {
		if (!touched(state, touchedSet, table.from, table.to)) {
			ranges.push(
				Decoration.replace({
					block: true,
					widget: new TableWidget(table, state.doc.sliceString(table.from, table.to))
				}).range(table.from, table.to)
			);
			continue;
		}
		const first = state.doc.lineAt(table.from).number;
		const last = state.doc.lineAt(table.to).number;
		eachLine(state, table.from, table.to, (at) => {
			const classes = ['cm-lp-table-raw'];
			if (at.number === first) classes.push('cm-lp-table-raw-first');
			if (at.number === first + 1) classes.push('cm-lp-table-raw-delim');
			if (at.number === last) classes.push('cm-lp-table-raw-last');
			ranges.push(line(at.from, classes.join(' ')));
		});
	}
	return Decoration.set(ranges, true);
}

export const liveTables = StateField.define<DecorationSet>({
	create: (state) => tableDecorations(state),
	update(value, tr) {
		if (tr.docChanged || tr.selection || tr.effects.some((effect) => effect.is(refreshDecorations)))
			return tableDecorations(tr.state);
		return value.map(tr.changes);
	},
	provide: (field) => EditorView.decorations.from(field)
});

function selectCell(view: EditorView, table: TableModel, row: number, col: number): boolean {
	const cell = table.rows[row]?.cells[col];
	if (!cell) return false;
	view.dispatch({ selection: { anchor: cell.from, head: cell.to }, scrollIntoView: true });
	return true;
}

function locate(view: EditorView): { table: TableModel; row: number; col: number } | null {
	const head = view.state.selection.main.head;
	const table = tableAt(view.state, head);
	if (!table) return null;
	const cell = locateCell(table, head);
	return cell ? { table, ...cell } : null;
}

function nextCell(view: EditorView): boolean {
	const at = locate(view);
	if (!at) return false;
	if (at.col + 1 < at.table.cols) return selectCell(view, at.table, at.row, at.col + 1);
	if (at.row + 1 < at.table.rows.length) return selectCell(view, at.table, at.row + 1, 0);
	appendRow(view, at.table, at.col);
	return true;
}

function previousCell(view: EditorView): boolean {
	const at = locate(view);
	if (!at) return false;
	if (at.col > 0) return selectCell(view, at.table, at.row, at.col - 1);
	if (at.row > 0) return selectCell(view, at.table, at.row - 1, at.table.cols - 1);
	return true;
}

function enterCell(view: EditorView): boolean {
	const at = locate(view);
	if (!at) return false;
	if (at.row + 1 < at.table.rows.length) return selectCell(view, at.table, at.row + 1, at.col);
	appendRow(view, at.table, at.col);
	return true;
}

function leaveTable(view: EditorView): boolean {
	const at = locate(view);
	if (!at) return false;
	const doc = view.state.doc;
	if (doc.sliceString(at.table.to, at.table.to + 1) !== '\n') return true;
	view.dispatch({ selection: { anchor: at.table.to + 1 }, scrollIntoView: true });
	return true;
}

// Escape leaves the table only when no higher-priority feature is answering it.
class TableEscape {
	readonly release: () => void;

	constructor(view: EditorView) {
		this.release = onEscape(view, ESCAPE_TABLE, () => {
			const at = locate(view);
			if (!at) return false;
			leaveTable(view);
			return true;
		});
	}

	destroy(): void {
		this.release();
	}
}

export const tableEscape = ViewPlugin.fromClass(TableEscape);

export const tableKeymap = keymap.of([
	{ key: 'Tab', run: nextCell },
	{ key: 'Shift-Tab', run: previousCell },
	{ key: 'Enter', run: enterCell }
]);

export const liveTableEditing: Extension = [liveTables, tableEscape, tableKeymap];
