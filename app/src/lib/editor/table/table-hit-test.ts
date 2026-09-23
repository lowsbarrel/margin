import { cellAround } from '@tiptap/pm/tables';
import type { EditorView } from '@tiptap/pm/view';

import { getCellInfoAt, type HoveringCellInfo } from './table-query';

export type DraggingDOMs = { table: HTMLTableElement; cell: HTMLTableCellElement };

function domCellAround(target: HTMLElement | null): HTMLElement | null {
	while (target && target.nodeName !== 'TD' && target.nodeName !== 'TH') {
		target = target.classList?.contains('ProseMirror')
			? null
			: (target.parentNode as HTMLElement | null);
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
	return getCellInfoAt($cellPos);
}

function getTableDOMByPos(view: EditorView, pos: number): HTMLTableElement | undefined {
	const dom = view.domAtPos(pos).node;
	if (!dom) return;
	const element = dom instanceof HTMLElement ? dom : dom.parentElement;
	return element?.closest('table') ?? undefined;
}

function getTargetFirstCellDOM(
	table: HTMLTableElement,
	index: number,
	direction: 'row' | 'col'
): HTMLTableCellElement | undefined {
	if (direction === 'row') {
		const row = table.querySelectorAll('tr')[index];
		return row?.querySelector<HTMLTableCellElement>('th,td') ?? undefined;
	} else {
		const row = table.querySelector('tr');
		return row?.querySelectorAll<HTMLTableCellElement>('th,td')[index] ?? undefined;
	}
}

export function getDndRelatedDOMs(
	view: EditorView,
	cellPos: number | undefined,
	draggingIndex: number,
	direction: 'row' | 'col'
): DraggingDOMs | undefined {
	if (cellPos == null) return;
	const table = getTableDOMByPos(view, cellPos);
	if (!table) return;
	const cell = getTargetFirstCellDOM(table, draggingIndex, direction);
	if (!cell) return;
	return { table, cell };
}

export type DragOverTarget = [Element, number];

function findDragOverElement(
	elements: Element[],
	pointer: number,
	axis: 'x' | 'y'
): DragOverTarget | undefined {
	const startProp = axis === 'x' ? 'left' : 'top';
	const endProp = axis === 'x' ? 'right' : 'bottom';
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

export function getDragOverColumn(
	table: HTMLTableElement,
	pointerX: number
): DragOverTarget | undefined {
	const firstRow = table.querySelector('tr');
	if (!firstRow) return;
	return findDragOverElement(Array.from(firstRow.children), pointerX, 'x');
}

export function getDragOverRow(
	table: HTMLTableElement,
	pointerY: number
): DragOverTarget | undefined {
	return findDragOverElement(Array.from(table.querySelectorAll('tr')), pointerY, 'y');
}
