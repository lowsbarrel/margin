/**
 * Table DnD utilities — row/column move logic, matrix helpers, cell queries.
 * Adapted from Docmost (Apache 2.0 / Atlassian editor-tables).
 */

import type { Node, ResolvedPos } from "@tiptap/pm/model";
import type { Selection, Transaction } from "@tiptap/pm/state";
import { CellSelection, TableMap, cellAround, cellNear, inSameTable } from "@tiptap/pm/tables";
import type { EditorView } from "@tiptap/pm/view";

// ─── Types ──────────────────────────────────────────────────────────────

export type CellPos = { pos: number; start: number; depth: number; node: Node };

export type CellSelectionRange = {
  $anchor: ResolvedPos;
  $head: ResolvedPos;
  indexes: number[];
};

export interface HoveringCellInfo {
  rowIndex: number;
  colIndex: number;
  cellPos: number;
  rowFirstCellPos: number;
  colFirstCellPos: number;
}

export type DraggingDOMs = { table: HTMLTableElement; cell: HTMLTableCellElement };

// ─── Query helpers ──────────────────────────────────────────────────────

function isCellSelection(value: unknown): value is CellSelection {
  return value instanceof CellSelection;
}

interface FindParentNodeResult {
  node: Node;
  pos: number;
  start: number;
  depth: number;
}

function findParentNode(
  predicate: (node: Node) => boolean,
  $pos: ResolvedPos,
): FindParentNodeResult | undefined {
  for (let depth = $pos.depth; depth >= 0; depth -= 1) {
    const node = $pos.node(depth);
    if (predicate(node)) {
      const pos = depth === 0 ? 0 : $pos.before(depth);
      const start = $pos.start(depth);
      return { node, pos, start, depth };
    }
  }
}

export function findTable($pos: ResolvedPos): FindParentNodeResult | undefined {
  return findParentNode((node) => node.type.spec.tableRole === "table", $pos);
}

function findCellPos(doc: Node, pos: number): ResolvedPos | undefined {
  const $pos = doc.resolve(pos);
  return cellAround($pos) || cellNear($pos);
}

export function findCellRange(
  selection: Selection,
  anchorHit?: number,
  headHit?: number,
): [ResolvedPos, ResolvedPos] | undefined {
  if (anchorHit == null && headHit == null && isCellSelection(selection)) {
    return [selection.$anchorCell, selection.$headCell];
  }
  const anchor: number = anchorHit ?? headHit ?? selection.anchor;
  const head: number = headHit ?? anchorHit ?? selection.head;
  const doc = selection.$head.doc;
  const $anchorCell = findCellPos(doc, anchor);
  const $headCell = findCellPos(doc, head);
  if ($anchorCell && $headCell && inSameTable($anchorCell, $headCell)) {
    return [$anchorCell, $headCell];
  }
}

// ─── getCellsInRow / getCellsInColumn ───────────────────────────────────

export function getCellsInRow(rowIndex: number | number[], $from: ResolvedPos): CellPos[] | undefined {
  const table = findTable($from);
  if (!table) return;
  const map = TableMap.get(table.node);
  const indexes = Array.isArray(rowIndex) ? rowIndex : [rowIndex];
  return indexes
    .filter((i) => i >= 0 && i <= map.height - 1)
    .flatMap((index) => {
      const cells = map.cellsInRect({ left: 0, right: map.width, top: index, bottom: index + 1 });
      return cells.map((nodePos) => {
        const node = table.node.nodeAt(nodePos)!;
        const pos = nodePos + table.start;
        return { pos, start: pos + 1, node, depth: table.depth + 2 };
      });
    });
}

export function getCellsInColumn(columnIndexes: number | number[], $from: ResolvedPos): CellPos[] | undefined {
  const table = findTable($from);
  if (!table) return;
  const map = TableMap.get(table.node);
  const indexes = Array.isArray(columnIndexes) ? columnIndexes : [columnIndexes];
  return indexes
    .filter((i) => i >= 0 && i <= map.width - 1)
    .flatMap((index) => {
      const cells = map.cellsInRect({ left: index, right: index + 1, top: 0, bottom: map.height });
      return cells.map((nodePos) => {
        const node = table.node.nodeAt(nodePos)!;
        const pos = nodePos + table.start;
        return { pos, start: pos + 1, node, depth: table.depth + 2 };
      });
    });
}

// ─── Selection range (merged cell aware) ────────────────────────────────

export function getSelectionRangeInRow($from: ResolvedPos, startRowIndex: number, endRowIndex: number = startRowIndex): CellSelectionRange | undefined {
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

export function getSelectionRangeInColumn($from: ResolvedPos, startColIndex: number, endColIndex: number = startColIndex): CellSelectionRange | undefined {
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

  const $anchor = $from.doc.resolve(firstSelectedColumnCells[firstSelectedColumnCells.length - 1].pos);

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

// ─── Matrix conversion ─────────────────────────────────────────────────

export function convertTableNodeToArrayOfRows(tableNode: Node): (Node | null)[][] {
  const map = TableMap.get(tableNode);
  const rows: (Node | null)[][] = [];
  for (let r = 0; r < map.height; r++) {
    const row: (Node | null)[] = [];
    for (let c = 0; c < map.width; c++) {
      const cellIndex = r * map.width + c;
      const cellPos = map.map[cellIndex];
      if (r > 0 && cellPos === map.map[cellIndex - map.width]) { row.push(null); continue; }
      if (c > 0 && cellPos === map.map[cellIndex - 1]) { row.push(null); continue; }
      row.push(tableNode.nodeAt(cellPos));
    }
    rows.push(row);
  }
  return rows;
}

export function convertArrayOfRowsToTableNode(tableNode: Node, arrayOfNodes: (Node | null)[][]): Node {
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
      const newCell = oldCell.type.createChecked(Object.assign({}, cell.attrs), cell.content, cell.marks);
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
  directionOverride: -1 | 1 | 0,
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
    target = direction === -1
      ? indexesTarget[0]
      : indexesTarget[indexesTarget.length - 1] - positionOffset;
  }

  rows.splice(target, 0, ...rowsExtracted);
  return rows;
}

// ─── moveRow / moveColumn ───────────────────────────────────────────────

export interface MoveRowParams {
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

export interface MoveColumnParams {
  tr: Transaction;
  originIndex: number;
  targetIndex: number;
  select: boolean;
  pos: number;
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

// ─── DOM helpers for DnD ────────────────────────────────────────────────

function domCellAround(target: HTMLElement | null): HTMLElement | null {
  while (target && target.nodeName !== "TD" && target.nodeName !== "TH") {
    target = target.classList?.contains("ProseMirror") ? null : (target.parentNode as HTMLElement | null);
  }
  return target;
}

export function getHoveringCell(view: EditorView, event: MouseEvent): HoveringCellInfo | undefined {
  const domCell = domCellAround(event.target as HTMLElement | null);
  if (!domCell) return;
  const { left, top, width, height } = domCell.getBoundingClientRect();
  const eventPos = view.posAtCoords({ left: left + width / 2, top: top + height / 2 });
  if (!eventPos) return;
  const $cellPos = cellAround(view.state.doc.resolve(eventPos.pos));
  if (!$cellPos) return;
  const map = TableMap.get($cellPos.node(-1));
  const tableStart = $cellPos.start(-1);
  const cellRect = map.findCell($cellPos.pos - tableStart);
  const rowIndex = cellRect.top;
  const colIndex = cellRect.left;

  function getCellPosAt(r: number, c: number): number {
    const cellIndex = map.width * r + c;
    return tableStart + map.map[cellIndex];
  }

  return {
    rowIndex,
    colIndex,
    cellPos: $cellPos.pos,
    rowFirstCellPos: getCellPosAt(rowIndex, 0),
    colFirstCellPos: getCellPosAt(0, colIndex),
  };
}

function getTableDOMByPos(view: EditorView, pos: number): HTMLTableElement | undefined {
  const dom = view.domAtPos(pos).node;
  if (!dom) return;
  const element = dom instanceof HTMLElement ? dom : dom.parentElement;
  return element?.closest("table") ?? undefined;
}

function getTargetFirstCellDOM(table: HTMLTableElement, index: number, direction: "row" | "col"): HTMLTableCellElement | undefined {
  if (direction === "row") {
    const row = table.querySelectorAll("tr")[index];
    return row?.querySelector<HTMLTableCellElement>("th,td") ?? undefined;
  } else {
    const row = table.querySelector("tr");
    return row?.querySelectorAll<HTMLTableCellElement>("th,td")[index] ?? undefined;
  }
}

export function getDndRelatedDOMs(view: EditorView, cellPos: number | undefined, draggingIndex: number, direction: "row" | "col"): DraggingDOMs | undefined {
  if (cellPos == null) return;
  const table = getTableDOMByPos(view, cellPos);
  if (!table) return;
  const cell = getTargetFirstCellDOM(table, draggingIndex, direction);
  if (!cell) return;
  return { table, cell };
}

// ─── Drag-over detection ────────────────────────────────────────────────

function findDragOverElement(elements: Element[], pointer: number, axis: "x" | "y"): [Element, number] | undefined {
  const startProp = axis === "x" ? "left" : "top";
  const endProp = axis === "x" ? "right" : "bottom";
  const lastIndex = elements.length - 1;
  const index = elements.findIndex((el, i) => {
    const rect = el.getBoundingClientRect();
    if (rect[startProp] <= pointer && pointer <= rect[endProp]) return true;
    if (i === lastIndex && pointer > rect[endProp]) return true;
    if (i === 0 && pointer < rect[startProp]) return true;
    return false;
  });
  return index >= 0 ? [elements[index], index] : undefined;
}

export function getDragOverColumn(table: HTMLTableElement, pointerX: number): [Element, number] | undefined {
  const firstRow = table.querySelector("tr");
  if (!firstRow) return;
  return findDragOverElement(Array.from(firstRow.children), pointerX, "x");
}

export function getDragOverRow(table: HTMLTableElement, pointerY: number): [Element, number] | undefined {
  return findDragOverElement(Array.from(table.querySelectorAll("tr")), pointerY, "y");
}
