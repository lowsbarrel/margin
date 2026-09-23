import type { ResolvedPos } from '@tiptap/pm/model';

import { getCellsInColumn, getCellsInRow, type CellPos } from './table-query';

export interface CellSelectionRange {
	$anchor: ResolvedPos;
	$head: ResolvedPos;
	indexes: number[];
}

export function getSelectionRangeInRow(
	$from: ResolvedPos,
	startRowIndex: number,
	endRowIndex: number = startRowIndex
): CellSelectionRange | undefined {
	let startIndex = startRowIndex;
	let endIndex = endRowIndex;

	for (let i = startRowIndex; i >= 0; i--) {
		const cells = getCellsInRow(i, $from);
		if (cells) {
			cells.forEach((cell) => {
				const maybeEnd = cell.node.attrs.rowspan + i - 1;
				if (maybeEnd >= startIndex) startIndex = i;
				if (maybeEnd > endIndex) endIndex = maybeEnd;
			});
		}
	}
	for (let i = startRowIndex; i <= endIndex; i++) {
		const cells = getCellsInRow(i, $from);
		if (cells) {
			cells.forEach((cell) => {
				const maybeEnd = cell.node.attrs.rowspan + i - 1;
				if (cell.node.attrs.rowspan > 1 && maybeEnd > endIndex) endIndex = maybeEnd;
			});
		}
	}

	const indexes: number[] = [];
	for (let i = startIndex; i <= endIndex; i++) {
		const c = getCellsInRow(i, $from);
		if (c && c.length > 0) indexes.push(i);
	}
	startIndex = indexes[0];
	endIndex = indexes[indexes.length - 1];

	const firstSelectedRowCells = getCellsInRow(startIndex, $from);
	const firstColumnCells = getCellsInColumn(0, $from);
	if (!firstSelectedRowCells || !firstColumnCells) return;

	const $anchor = $from.doc.resolve(firstSelectedRowCells[firstSelectedRowCells.length - 1].pos);

	let headCell: CellPos | undefined;
	for (let i = endIndex; i >= startIndex; i--) {
		const rowCells = getCellsInRow(i, $from);
		if (rowCells && rowCells.length > 0) {
			for (let j = firstColumnCells.length - 1; j >= 0; j--) {
				if (firstColumnCells[j].pos === rowCells[0].pos) {
					headCell = rowCells[0];
					break;
				}
			}
			if (headCell) break;
		}
	}
	if (!headCell) return;

	const $head = $from.doc.resolve(headCell.pos);
	return { $anchor, $head, indexes };
}

export function getSelectionRangeInColumn(
	$from: ResolvedPos,
	startColIndex: number,
	endColIndex: number = startColIndex
): CellSelectionRange | undefined {
	let startIndex = startColIndex;
	let endIndex = endColIndex;

	for (let i = startColIndex; i >= 0; i--) {
		const cells = getCellsInColumn(i, $from);
		if (cells) {
			cells.forEach((cell) => {
				const maybeEnd = cell.node.attrs.colspan + i - 1;
				if (maybeEnd >= startIndex) startIndex = i;
				if (maybeEnd > endIndex) endIndex = maybeEnd;
			});
		}
	}
	for (let i = startColIndex; i <= endIndex; i++) {
		const cells = getCellsInColumn(i, $from);
		if (cells) {
			cells.forEach((cell) => {
				const maybeEnd = cell.node.attrs.colspan + i - 1;
				if (cell.node.attrs.colspan > 1 && maybeEnd > endIndex) endIndex = maybeEnd;
			});
		}
	}

	const indexes: number[] = [];
	for (let i = startIndex; i <= endIndex; i++) {
		const c = getCellsInColumn(i, $from);
		if (c && c.length > 0) indexes.push(i);
	}
	startIndex = indexes[0];
	endIndex = indexes[indexes.length - 1];

	const firstSelectedColumnCells = getCellsInColumn(startIndex, $from);
	const firstRowCells = getCellsInRow(0, $from);
	if (!firstSelectedColumnCells || !firstRowCells) return;

	const $anchor = $from.doc.resolve(
		firstSelectedColumnCells[firstSelectedColumnCells.length - 1].pos
	);

	let headCell: CellPos | undefined;
	for (let i = endIndex; i >= startIndex; i--) {
		const columnCells = getCellsInColumn(i, $from);
		if (columnCells && columnCells.length > 0) {
			for (let j = firstRowCells.length - 1; j >= 0; j--) {
				if (firstRowCells[j].pos === columnCells[0].pos) {
					headCell = columnCells[0];
					break;
				}
			}
			if (headCell) break;
		}
	}
	if (!headCell) return;

	const $head = $from.doc.resolve(headCell.pos);
	return { $anchor, $head, indexes };
}
