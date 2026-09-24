import type { EditorView } from '@codemirror/view';
import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import * as m from '$lib/paraglide/messages.js';
import { contextOf } from './context';
import { tableAt, type TableModel } from './table-model';
import {
	alignColumn,
	deleteColumn,
	deleteRow,
	insertColumn,
	insertRow,
	moveColumnTo,
	moveRowTo
} from './table-ops';

function run(view: EditorView, anchor: number, action: (table: TableModel) => void) {
	return () => {
		const table = tableAt(view.state, anchor);
		if (table) action(table);
	};
}

export function openRowMenu(
	view: EditorView,
	anchor: number,
	row: number,
	at: { x: number; y: number }
): void {
	const table = tableAt(view.state, anchor);
	if (!table) return;
	const items: ContextMenuItem[] = [
		{
			label: m.editor_table_insert_row_above(),
			onclick: run(view, anchor, (current) => insertRow(view, current, row, 'above'))
		},
		{
			label: m.editor_table_insert_row_below(),
			onclick: run(view, anchor, (current) => insertRow(view, current, row, 'below'))
		},
		{
			label: m.editor_table_move_row_up(),
			disabled: row === 0,
			onclick: run(view, anchor, (current) => moveRowTo(view, current, row, row - 1))
		},
		{
			label: m.editor_table_move_row_down(),
			disabled: row >= table.rows.length - 1,
			onclick: run(view, anchor, (current) => moveRowTo(view, current, row, row + 1))
		},
		{
			label: m.editor_table_delete_row(),
			destructive: true,
			disabled: table.rows.length <= 1,
			onclick: run(view, anchor, (current) => deleteRow(view, current, row))
		}
	];
	contextOf(view.state).openContextMenu(at.x, at.y, items);
}

export function openColumnMenu(
	view: EditorView,
	anchor: number,
	col: number,
	at: { x: number; y: number }
): void {
	const table = tableAt(view.state, anchor);
	if (!table) return;
	const items: ContextMenuItem[] = [
		{
			label: m.editor_table_insert_column_left(),
			onclick: run(view, anchor, (current) => insertColumn(view, current, col, 'left'))
		},
		{
			label: m.editor_table_insert_column_right(),
			onclick: run(view, anchor, (current) => insertColumn(view, current, col, 'right'))
		},
		{
			label: m.editor_table_move_column_left(),
			disabled: col === 0,
			onclick: run(view, anchor, (current) => moveColumnTo(view, current, col, col - 1))
		},
		{
			label: m.editor_table_move_column_right(),
			disabled: col >= table.cols - 1,
			onclick: run(view, anchor, (current) => moveColumnTo(view, current, col, col + 1))
		},
		{
			label: m.editor_table_align_left(),
			onclick: run(view, anchor, (current) => alignColumn(view, current, col, 'left'))
		},
		{
			label: m.editor_table_align_center(),
			onclick: run(view, anchor, (current) => alignColumn(view, current, col, 'center'))
		},
		{
			label: m.editor_table_align_right(),
			onclick: run(view, anchor, (current) => alignColumn(view, current, col, 'right'))
		},
		{
			label: m.editor_table_delete_column(),
			destructive: true,
			disabled: table.cols <= 1,
			onclick: run(view, anchor, (current) => deleteColumn(view, current, col))
		}
	];
	contextOf(view.state).openContextMenu(at.x, at.y, items);
}
