import { Annotation, StateEffect, StateField, type EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { tableAt, type TableModel } from './table-model';
import { rowsOf, writeTable } from './table-ops';

export interface CellPoint {
	row: number;
	col: number;
}

export interface CellRange {
	table: number;
	anchor: CellPoint;
	head: CellPoint;
}

export const setCellRange = StateEffect.define<CellRange | null>();
const rangeEdit = Annotation.define<boolean>();

export const cellRange = StateField.define<CellRange | null>({
	create: () => null,
	update(value, tr) {
		for (const effect of tr.effects) if (effect.is(setCellRange)) return effect.value;
		if (!value) return null;
		if (tr.docChanged && !tr.annotation(rangeEdit)) return null;
		if (!tr.docChanged && tr.selection && !tr.selection.eq(tr.startState.selection)) return null;
		return tr.docChanged ? { ...value, table: tr.changes.mapPos(value.table) } : value;
	}
});

export function sameRange(a: CellRange | null, b: CellRange | null): boolean {
	if (!a || !b) return a === b;
	return (
		a.table === b.table &&
		a.anchor.row === b.anchor.row &&
		a.anchor.col === b.anchor.col &&
		a.head.row === b.head.row &&
		a.head.col === b.head.col
	);
}

function bounds(range: CellRange) {
	return {
		top: Math.min(range.anchor.row, range.head.row),
		bottom: Math.max(range.anchor.row, range.head.row),
		left: Math.min(range.anchor.col, range.head.col),
		right: Math.max(range.anchor.col, range.head.col)
	};
}

export function inRange(range: CellRange | null, row: number, col: number): boolean {
	if (!range) return false;
	const { top, bottom, left, right } = bounds(range);
	return row >= top && row <= bottom && col >= left && col <= right;
}

function cellsIn(range: CellRange): CellPoint[] {
	const { top, bottom, left, right } = bounds(range);
	const cells: CellPoint[] = [];
	for (let row = top; row <= bottom; row++)
		for (let col = left; col <= right; col++) cells.push({ row, col });
	return cells;
}

function rangeTable(view: EditorView): { range: CellRange; table: TableModel } | null {
	const range = view.state.field(cellRange, false);
	const table = range && tableAt(view.state, range.table);
	return range && table ? { range, table } : null;
}

function rewriteCells(view: EditorView, edit: (text: string, all: string[]) => string): boolean {
	const found = rangeTable(view);
	if (!found) return false;
	const rows = rowsOf(found.table);
	const cells = cellsIn(found.range).filter(({ row, col }) => rows[row]?.[col] !== undefined);
	const texts = cells.map(({ row, col }) => rows[row][col]);
	for (const { row, col } of cells) rows[row][col] = edit(rows[row][col], texts);
	writeTable(
		view,
		found.table,
		{ rows, align: found.table.align, select: found.range.anchor },
		{ annotations: rangeEdit.of(true) }
	);
	return true;
}

export function clearCells(view: EditorView): boolean {
	return rewriteCells(view, () => '');
}

function wrapped(text: string, marker: string): boolean {
	if (text.length <= marker.length * 2 || !text.startsWith(marker) || !text.endsWith(marker))
		return false;
	return marker !== '*' || (!text.startsWith('**') && !text.endsWith('**'));
}

export function formatCells(view: EditorView, marker: string): boolean {
	return rewriteCells(view, (text, all) => {
		if (!text) return text;
		const unwrap = all.filter(Boolean).every((cell) => wrapped(cell, marker));
		return unwrap ? text.slice(marker.length, -marker.length) : `${marker}${text}${marker}`;
	});
}

function rangeMarkdown(state: EditorState, range: CellRange): string | null {
	const table = tableAt(state, range.table);
	if (!table) return null;
	const { top, bottom, left, right } = bounds(range);
	const cols = Array.from({ length: right - left + 1 }, (_, i) => left + i);
	const rows: string[] = [];
	for (let row = top; row <= bottom; row++) {
		const cells = cols.map((col) => table.rows[row]?.cells[col]?.text ?? '');
		rows.push(`| ${cells.join(' | ')} |`);
	}
	const delimiter = `| ${cols.map(() => '---').join(' | ')} |`;
	return [rows[0], delimiter, ...rows.slice(1)].join('\n');
}

export function copyCells(view: EditorView, event: ClipboardEvent, cut: boolean): boolean {
	const range = view.state.field(cellRange, false);
	const text = range && rangeMarkdown(view.state, range);
	if (!text || !event.clipboardData) return false;
	event.clipboardData.setData('text/plain', text);
	event.preventDefault();
	if (cut) clearCells(view);
	return true;
}

export function dropRange(view: EditorView): boolean {
	if (!view.state.field(cellRange, false)) return false;
	view.dispatch({ effects: setCellRange.of(null) });
	return true;
}

export function rangeColumns(state: EditorState, table: TableModel, col: number): number[] {
	const range = state.field(cellRange, false);
	if (!range || range.table !== table.from) return [col];
	const { left, right } = bounds(range);
	if (col < left || col > right) return [col];
	return Array.from({ length: right - left + 1 }, (_, i) => left + i);
}

export function selectTable(view: EditorView, table: TableModel): void {
	const head = { row: table.rows.length - 1, col: table.cols - 1 };
	view.dispatch({
		selection: { anchor: table.from, head: table.to },
		effects: setCellRange.of({ table: table.from, anchor: { row: 0, col: 0 }, head })
	});
}

export function selectTableAbove(view: EditorView): boolean {
	const range = view.state.selection.main;
	if (!range.empty) return false;
	const line = view.state.doc.lineAt(range.head);
	if (range.head !== line.from || line.number === 1) return false;
	const table = tableAt(view.state, line.from - 1);
	if (!table || table.to !== line.from - 1) return false;
	selectTable(view, table);
	return true;
}

export function extendRange(view: EditorView, table: number, from: CellPoint, head: CellPoint) {
	const current = view.state.field(cellRange, false);
	const anchor = current?.table === table ? current.anchor : { row: from.row, col: from.col };
	view.dispatch({ effects: setCellRange.of({ table, anchor, head }) });
}

export function trackRange(
	view: EditorView,
	table: number,
	anchor: CellPoint,
	locate: (target: Element | null) => CellPoint | null,
	begin: () => void
): void {
	const controller = new AbortController();
	let last = anchor;
	let started = false;
	const move = (event: MouseEvent) => {
		const head = locate(document.elementFromPoint(event.clientX, event.clientY));
		if (!head || (head.row === last.row && head.col === last.col)) return;
		last = head;
		if (!started) begin();
		started = true;
		const same = head.row === anchor.row && head.col === anchor.col;
		view.dispatch({ effects: setCellRange.of(same ? null : { table, anchor, head }) });
	};
	window.addEventListener('mousemove', move, { signal: controller.signal });
	window.addEventListener('mouseup', () => controller.abort(), { signal: controller.signal });
}
