import type { Node } from '@tiptap/pm/model';
import type { Transaction } from '@tiptap/pm/state';
import { CellSelection, TableMap } from '@tiptap/pm/tables';

import { findTable } from './table-query';
import { getSelectionRangeInColumn, getSelectionRangeInRow } from './table-selection';

export function convertTableNodeToArrayOfRows(tableNode: Node): (Node | null)[][] {
	const map = TableMap.get(tableNode);
	const rows: (Node | null)[][] = [];
	for (let r = 0; r < map.height; r++) {
		const row: (Node | null)[] = [];
		for (let c = 0; c < map.width; c++) {
			const cellIndex = r * map.width + c;
			const cellPos = map.map[cellIndex];
			if (r > 0 && cellPos === map.map[cellIndex - map.width]) {
				row.push(null);
				continue;
			}
			if (c > 0 && cellPos === map.map[cellIndex - 1]) {
				row.push(null);
				continue;
			}
			row.push(tableNode.nodeAt(cellPos));
		}
		rows.push(row);
	}
	return rows;
}

export function convertArrayOfRowsToTableNode(
	tableNode: Node,
	arrayOfNodes: (Node | null)[][]
): Node {
	const map = TableMap.get(tableNode);
	const rowsPM = [];
	for (let r = 0; r < map.height; r++) {
		const row = tableNode.child(r);
		const rowCells = [];
		for (let c = 0; c < map.width; c++) {
			if (!arrayOfNodes[r][c]) continue;
			const cellPos = map.map[r * map.width + c];
			const cell = arrayOfNodes[r][c]!;
			const oldCell = tableNode.nodeAt(cellPos)!;
			const newCell = oldCell.type.createChecked(
				Object.assign({}, cell.attrs),
				cell.content,
				cell.marks
			);
			rowCells.push(newCell);
		}
		rowsPM.push(row.type.createChecked(row.attrs, rowCells, row.marks));
	}
	return tableNode.type.createChecked(tableNode.attrs, rowsPM, tableNode.marks);
}

function transpose<T>(array: T[][]): T[][] {
	return array[0].map((_, i) => array.map((col) => col[i]));
}

export function moveRowInArrayOfRows<T>(
	rows: T[],
	indexesOrigin: number[],
	indexesTarget: number[],
	directionOverride: -1 | 1 | 0
): T[] {
	const direction = indexesOrigin[0] > indexesTarget[0] ? -1 : 1;
	const rowsExtracted = rows.splice(indexesOrigin[0], indexesOrigin.length);
	const positionOffset = rowsExtracted.length % 2 === 0 ? 1 : 0;
	let target: number;

	if (directionOverride === -1 && direction === 1) {
		target = indexesTarget[0] - 1;
	} else if (directionOverride === 1 && direction === -1) {
		target = indexesTarget[indexesTarget.length - 1] - positionOffset + 1;
	} else {
		target =
			direction === -1
				? indexesTarget[0]
				: indexesTarget[indexesTarget.length - 1] - positionOffset;
	}

	rows.splice(target, 0, ...rowsExtracted);
	return rows;
}

export interface MoveRowParams {
	tr: Transaction;
	originIndex: number;
	targetIndex: number;
	select: boolean;
	pos: number;
}

export interface MoveColumnParams {
	tr: Transaction;
	originIndex: number;
	targetIndex: number;
	select: boolean;
	pos: number;
}

export function moveRow(params: MoveRowParams): boolean {
	const { tr, originIndex, targetIndex, select, pos } = params;
	const $pos = tr.doc.resolve(pos);
	const table = findTable($pos);
	if (!table) return false;

	const indexesOriginRow = getSelectionRangeInRow($pos, originIndex)?.indexes;
	const indexesTargetRow = getSelectionRangeInRow($pos, targetIndex)?.indexes;
	if (!indexesOriginRow || !indexesTargetRow) return false;
	if (indexesOriginRow.includes(targetIndex)) return false;

	let rows = convertTableNodeToArrayOfRows(table.node);
	rows = moveRowInArrayOfRows(rows, indexesOriginRow, indexesTargetRow, 0);
	const newTable = convertArrayOfRowsToTableNode(table.node, rows);

	tr.replaceWith(table.pos, table.pos + table.node.nodeSize, newTable);

	if (!select) return true;

	const map = TableMap.get(newTable);
	const start = table.start;
	const lastCell = map.positionAt(targetIndex, map.width - 1, newTable);
	const $lastCell = tr.doc.resolve(start + lastCell);
	const firstCell = map.positionAt(targetIndex, 0, newTable);
	const $firstCell = tr.doc.resolve(start + firstCell);
	tr.setSelection(CellSelection.rowSelection($lastCell, $firstCell));
	return true;
}

export function moveColumn(params: MoveColumnParams): boolean {
	const { tr, originIndex, targetIndex, select, pos } = params;
	const $pos = tr.doc.resolve(pos);
	const table = findTable($pos);
	if (!table) return false;

	const indexesOriginColumn = getSelectionRangeInColumn($pos, originIndex)?.indexes;
	const indexesTargetColumn = getSelectionRangeInColumn($pos, targetIndex)?.indexes;
	if (!indexesOriginColumn || !indexesTargetColumn) return false;
	if (indexesOriginColumn.includes(targetIndex)) return false;

	let rows = transpose(convertTableNodeToArrayOfRows(table.node));
	rows = moveRowInArrayOfRows(rows, indexesOriginColumn, indexesTargetColumn, 0);
	rows = transpose(rows);
	const newTable = convertArrayOfRowsToTableNode(table.node, rows);

	tr.replaceWith(table.pos, table.pos + table.node.nodeSize, newTable);

	if (!select) return true;

	const map = TableMap.get(newTable);
	const start = table.start;
	const lastCell = map.positionAt(map.height - 1, targetIndex, newTable);
	const $lastCell = tr.doc.resolve(start + lastCell);
	const firstCell = map.positionAt(0, targetIndex, newTable);
	const $firstCell = tr.doc.resolve(start + firstCell);
	tr.setSelection(CellSelection.colSelection($lastCell, $firstCell));
	return true;
}
