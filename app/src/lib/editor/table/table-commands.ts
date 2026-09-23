import type { Editor } from '@tiptap/core';
import { TextSelection } from '@tiptap/pm/state';
import { CellSelection, TableMap, cellAround } from '@tiptap/pm/tables';
import type { EditorView } from '@tiptap/pm/view';
import * as m from '$lib/paraglide/messages.js';

import { openTableMenu, type TableMenuCloseReason, type TableMenuItem } from '../table-menu';
import type { TableHandles } from './table-handles';
import { findTable, getCellInfoAt, type HoveringCellInfo, type TablePosition } from './table-query';
import { getSelectionRangeInColumn, getSelectionRangeInRow } from './table-selection';
import { moveColumn, moveRow } from './table-move';

export interface TableGripCommands {
	openMenu(kind: 'col' | 'row', cell: HoveringCellInfo): void;
	openMenuFromSelection(kind: 'col' | 'row'): void;
	appendRow(): void;
	appendColumn(): void;
	applyMove(kind: 'col' | 'row', pos: number, originIndex: number, targetIndex: number): boolean;
}

interface TableCommandOptions {
	view: EditorView;
	editor: Editor;
	handles: TableHandles;
	hoveringCell: () => HoveringCellInfo | undefined;
	onMenuOpenChange: (open: boolean) => void;
	onMenuClose: (reason: TableMenuCloseReason) => void;
}

export function createTableCommands(options: TableCommandOptions): TableGripCommands {
	const { view, editor, handles, hoveringCell, onMenuOpenChange, onMenuClose } = options;

	function selectGrip(kind: 'col' | 'row', cell: HoveringCellInfo): boolean {
		const $pos = view.state.doc.resolve(cell.cellPos);
		const range =
			kind === 'row'
				? getSelectionRangeInRow($pos, cell.rowIndex)
				: getSelectionRangeInColumn($pos, cell.colIndex);
		if (!range) return false;
		const selection =
			kind === 'row'
				? CellSelection.rowSelection(range.$anchor, range.$head)
				: CellSelection.colSelection(range.$anchor, range.$head);
		view.dispatch(view.state.tr.setSelection(selection));
		return true;
	}

	function applyMove(
		kind: 'col' | 'row',
		pos: number,
		originIndex: number,
		targetIndex: number
	): boolean {
		const tr = editor.state.tr;
		const moved =
			kind === 'col'
				? moveColumn({ tr, originIndex, targetIndex, select: true, pos })
				: moveRow({ tr, originIndex, targetIndex, select: true, pos });
		if (!moved) return false;
		view.dispatch(tr);
		return true;
	}

	function moveGrip(kind: 'col' | 'row', cell: HoveringCellInfo, target: number) {
		const origin = kind === 'col' ? cell.colIndex : cell.rowIndex;
		if (applyMove(kind, cell.cellPos, origin, target)) view.focus();
	}

	function gripperMenuItems(
		kind: 'col' | 'row',
		cell: HoveringCellInfo,
		map: TableMap
	): TableMenuItem[] {
		const chain = () => editor.chain().focus();
		if (kind === 'row') {
			return [
				{
					label: m.editor_table_insert_row_above(),
					onclick: () => {
						chain().addRowBefore().run();
					}
				},
				{
					label: m.editor_table_insert_row_below(),
					onclick: () => {
						chain().addRowAfter().run();
					}
				},
				{
					label: m.editor_table_move_row_up(),
					disabled: cell.rowIndex === 0,
					onclick: () => moveGrip('row', cell, cell.rowIndex - 1)
				},
				{
					label: m.editor_table_move_row_down(),
					disabled: cell.rowIndex >= map.height - 1,
					onclick: () => moveGrip('row', cell, cell.rowIndex + 1)
				},
				{
					label: m.editor_table_toggle_header_row(),
					onclick: () => {
						chain().toggleHeaderRow().run();
					}
				},
				{
					label: m.editor_table_delete_row(),
					destructive: true,
					onclick: () => {
						chain().deleteRow().run();
					}
				},
				{
					label: m.editor_table_delete_table(),
					destructive: true,
					onclick: () => {
						chain().deleteTable().run();
					}
				}
			];
		}
		return [
			{
				label: m.editor_table_insert_column_left(),
				onclick: () => {
					chain().addColumnBefore().run();
				}
			},
			{
				label: m.editor_table_insert_column_right(),
				onclick: () => {
					chain().addColumnAfter().run();
				}
			},
			{
				label: m.editor_table_move_column_left(),
				disabled: cell.colIndex === 0,
				onclick: () => moveGrip('col', cell, cell.colIndex - 1)
			},
			{
				label: m.editor_table_move_column_right(),
				disabled: cell.colIndex >= map.width - 1,
				onclick: () => moveGrip('col', cell, cell.colIndex + 1)
			},
			{
				label: m.editor_table_align_left(),
				onclick: () => {
					chain().setCellAttribute('align', 'left').run();
				}
			},
			{
				label: m.editor_table_align_center(),
				onclick: () => {
					chain().setCellAttribute('align', 'center').run();
				}
			},
			{
				label: m.editor_table_align_right(),
				onclick: () => {
					chain().setCellAttribute('align', 'right').run();
				}
			},
			{
				label: m.editor_table_delete_column(),
				destructive: true,
				onclick: () => {
					chain().deleteColumn().run();
				}
			},
			{
				label: m.editor_table_delete_table(),
				destructive: true,
				onclick: () => {
					chain().deleteTable().run();
				}
			}
		];
	}

	function openMenu(kind: 'col' | 'row', cell: HoveringCellInfo) {
		const table = findTable(view.state.doc.resolve(cell.cellPos));
		if (!table) return;
		if (!selectGrip(kind, cell)) return;
		onMenuOpenChange(true);
		openTableMenu({
			anchor: kind === 'row' ? handles.rowHandle : handles.colHandle,
			placement: kind === 'row' ? 'right-start' : 'bottom-start',
			label: kind === 'row' ? m.editor_table_row_menu() : m.editor_table_column_menu(),
			items: gripperMenuItems(kind, cell, TableMap.get(table.node)),
			onclose: (reason) => {
				onMenuOpenChange(false);
				onMenuClose(reason);
			}
		});
	}

	function caretCell(): HoveringCellInfo | undefined {
		const selection = view.state.selection;
		const $cell =
			selection instanceof CellSelection ? selection.$anchorCell : cellAround(selection.$from);
		return $cell ? getCellInfoAt($cell) : undefined;
	}

	function openMenuFromSelection(kind: 'col' | 'row') {
		const cell = caretCell();
		if (cell) openMenu(kind, cell);
	}

	function tableForAppend(): TablePosition | undefined {
		const cell = hoveringCell() ?? caretCell();
		if (!cell) return;
		return findTable(view.state.doc.resolve(cell.cellPos));
	}

	function placeCaret(target: number) {
		const tr = view.state.tr.setSelection(TextSelection.near(view.state.doc.resolve(target + 1)));
		view.dispatch(tr);
	}

	// The append commands act on a cell selection, so the caret is placed in the last row/column first.
	function appendRow() {
		const table = tableForAppend();
		if (!table) return;
		const map = TableMap.get(table.node);
		placeCaret(table.start + map.positionAt(map.height - 1, 0, table.node));
		editor.chain().focus().addRowAfter().run();
		handles.hideGrips();
		handles.repositionAddHandles(table.pos);
	}

	function appendColumn() {
		const table = tableForAppend();
		if (!table) return;
		const map = TableMap.get(table.node);
		placeCaret(table.start + map.positionAt(0, map.width - 1, table.node));
		editor.chain().focus().addColumnAfter().run();
		handles.hideGrips();
		handles.repositionAddHandles(table.pos);
	}

	return {
		openMenu,
		openMenuFromSelection,
		appendRow,
		appendColumn,
		applyMove
	};
}
