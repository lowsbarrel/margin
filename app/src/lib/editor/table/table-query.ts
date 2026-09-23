import type { Node, ResolvedPos } from '@tiptap/pm/model';
import { TableMap } from '@tiptap/pm/tables';

export type CellPos = { pos: number; start: number; depth: number; node: Node };

export interface HoveringCellInfo {
	rowIndex: number;
	colIndex: number;
	cellPos: number;
	rowFirstCellPos: number;
	colFirstCellPos: number;
}

export interface TablePosition {
	node: Node;
	pos: number;
	start: number;
	depth: number;
}

export interface TableRect {
	left: number;
	right: number;
	top: number;
	bottom: number;
}

function findParentNode(
	predicate: (node: Node) => boolean,
	$pos: ResolvedPos
): TablePosition | undefined {
	for (let depth = $pos.depth; depth >= 0; depth -= 1) {
		const node = $pos.node(depth);
		if (predicate(node)) {
			const pos = depth === 0 ? 0 : $pos.before(depth);
			const start = $pos.start(depth);
			return { node, pos, start, depth };
		}
	}
}

export function findTable($pos: ResolvedPos): TablePosition | undefined {
	return findParentNode((node) => node.type.spec.tableRole === 'table', $pos);
}

function cellsInRect(table: TablePosition, rect: TableRect): CellPos[] {
	const map = TableMap.get(table.node);
	return map.cellsInRect(rect).map((nodePos) => {
		const node = table.node.nodeAt(nodePos)!;
		const pos = nodePos + table.start;
		return { pos, start: pos + 1, node, depth: table.depth + 2 };
	});
}

export function getCellsInRow(
	rowIndex: number | number[],
	$from: ResolvedPos
): CellPos[] | undefined {
	const table = findTable($from);
	if (!table) return;
	const map = TableMap.get(table.node);
	const indexes = Array.isArray(rowIndex) ? rowIndex : [rowIndex];
	return indexes
		.filter((i) => i >= 0 && i <= map.height - 1)
		.flatMap((index) =>
			cellsInRect(table, { left: 0, right: map.width, top: index, bottom: index + 1 })
		);
}

export function getCellsInColumn(
	columnIndexes: number | number[],
	$from: ResolvedPos
): CellPos[] | undefined {
	const table = findTable($from);
	if (!table) return;
	const map = TableMap.get(table.node);
	const indexes = Array.isArray(columnIndexes) ? columnIndexes : [columnIndexes];
	return indexes
		.filter((i) => i >= 0 && i <= map.width - 1)
		.flatMap((index) =>
			cellsInRect(table, { left: index, right: index + 1, top: 0, bottom: map.height })
		);
}

export function getCellInfoAt($cell: ResolvedPos): HoveringCellInfo {
	const map = TableMap.get($cell.node(-1));
	const tableStart = $cell.start(-1);
	const rect = map.findCell($cell.pos - tableStart);

	function getCellPosAt(r: number, c: number): number {
		const cellIndex = map.width * r + c;
		return tableStart + map.map[cellIndex];
	}

	return {
		rowIndex: rect.top,
		colIndex: rect.left,
		cellPos: $cell.pos,
		rowFirstCellPos: getCellPosAt(rect.top, 0),
		colFirstCellPos: getCellPosAt(0, rect.left)
	};
}
