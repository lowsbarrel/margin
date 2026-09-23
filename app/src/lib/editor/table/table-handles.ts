import { computePosition, offset, type Placement } from '@floating-ui/dom';
import type { EditorView } from '@tiptap/pm/view';
import * as m from '$lib/paraglide/messages.js';

import { getTableUiLayer, releaseTableUiLayer } from '../table-ui-layer';
import type { HoveringCellInfo } from './table-query';

const HANDLE_OFFSET = -9;
// floating-ui shifts a floating element against the sign of its main-axis offset, so this is +24: away from the cell, clearing the block drag handle content-drag puts 24px left of a block.
const ROW_HANDLE_OFFSET = 24;
const HANDLE_GAP = 4;
const HIDE_DELAY = 140;

export interface TableHandles {
	colHandle: HTMLButtonElement;
	rowHandle: HTMLButtonElement;
	addRowHandle: HTMLButtonElement;
	addColumnHandle: HTMLButtonElement;
	showForCell(cell: HoveringCellInfo): void;
	showAddHandles(table: HTMLTableElement): void;
	repositionAddHandles(tablePos: number): void;
	hide(): void;
	hideGrips(): void;
	scheduleHide(): void;
	cancelHide(): void;
	destroy(): void;
}

export function createTableHandles(view: EditorView, isBusy: () => boolean): TableHandles {
	const layer = getTableUiLayer();

	function createHandle(kind: 'col' | 'row'): HTMLButtonElement {
		const el = document.createElement('button');
		el.type = 'button';
		el.className = 'table-drag-handle';
		el.dataset.handleType = kind;
		el.dataset.direction = kind === 'col' ? 'horizontal' : 'vertical';
		el.setAttribute(
			'aria-label',
			kind === 'col' ? m.editor_table_column_menu() : m.editor_table_row_menu()
		);
		return el;
	}

	function createAddHandle(axis: 'row' | 'col'): HTMLButtonElement {
		const el = document.createElement('button');
		el.type = 'button';
		el.className = 'table-add-handle';
		el.dataset.axis = axis;
		el.textContent = '+';
		el.setAttribute(
			'aria-label',
			axis === 'row' ? m.editor_table_add_row() : m.editor_table_add_column()
		);
		return el;
	}

	const colHandle = createHandle('col');
	const rowHandle = createHandle('row');
	const addRowHandle = createAddHandle('row');
	const addColumnHandle = createAddHandle('col');
	const handles = [colHandle, rowHandle, addRowHandle, addColumnHandle];

	layer.append(...handles);

	const pointerOverHandle = new Set<HTMLElement>();
	let hideTimer: number | undefined;

	function cancelHide() {
		if (hideTimer !== undefined) {
			window.clearTimeout(hideTimer);
			hideTimer = undefined;
		}
	}

	function hide() {
		pointerOverHandle.clear();
		for (const el of handles) el.classList.remove('is-visible');
	}

	function scheduleHide() {
		cancelHide();
		hideTimer = window.setTimeout(() => {
			hideTimer = undefined;
			if (isBusy() || pointerOverHandle.size > 0) return;
			if (layer.contains(document.activeElement)) return;
			hide();
		}, HIDE_DELAY);
	}

	for (const el of handles) {
		el.addEventListener('pointerenter', () => {
			pointerOverHandle.add(el);
			cancelHide();
		});
		el.addEventListener('pointerleave', () => {
			pointerOverHandle.delete(el);
			scheduleHide();
		});
		el.addEventListener('focus', cancelHide);
		el.addEventListener('blur', scheduleHide);
	}

	function positionAt(
		reference: HTMLElement,
		floating: HTMLElement,
		placement: Placement,
		mainAxis: number
	) {
		computePosition(reference, floating, {
			strategy: 'fixed',
			placement,
			middleware: [offset(mainAxis)]
		}).then(({ x, y }) => {
			floating.style.left = `${x}px`;
			floating.style.top = `${y}px`;
		});
	}

	function showForCell(cell: HoveringCellInfo) {
		const colRef = view.nodeDOM(cell.colFirstCellPos);
		if (colRef instanceof HTMLElement) {
			colHandle.classList.add('is-visible');
			positionAt(colRef, colHandle, 'top', HANDLE_OFFSET);
		}
		const rowRef = view.nodeDOM(cell.rowFirstCellPos);
		if (rowRef instanceof HTMLElement) {
			rowHandle.classList.add('is-visible');
			positionAt(rowRef, rowHandle, 'left', ROW_HANDLE_OFFSET);
		}
	}

	function showAddHandles(table: HTMLTableElement) {
		addRowHandle.classList.add('is-visible');
		addColumnHandle.classList.add('is-visible');
		positionAt(table, addRowHandle, 'bottom', HANDLE_GAP);
		positionAt(table, addColumnHandle, 'right', HANDLE_GAP);
	}

	function repositionAddHandles(tablePos: number) {
		const dom = view.nodeDOM(tablePos);
		if (!(dom instanceof HTMLElement)) return;
		const table = dom.matches('table') ? dom : dom.querySelector('table');
		if (table instanceof HTMLTableElement) showAddHandles(table);
	}

	function hideGrips() {
		colHandle.classList.remove('is-visible');
		rowHandle.classList.remove('is-visible');
	}

	function destroy() {
		cancelHide();
		for (const el of handles) el.remove();
		releaseTableUiLayer();
	}

	return {
		colHandle,
		rowHandle,
		addRowHandle,
		addColumnHandle,
		showForCell,
		showAddHandles,
		repositionAddHandles,
		hide,
		hideGrips,
		scheduleHide,
		cancelHide,
		destroy
	};
}
