import type { EditorView } from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import type { TableModel } from './table-model';
import { openColumnMenu, openRowMenu } from './table-menu';
import { appendRow, insertColumn, moveColumnTo, moveRowTo } from './table-ops';

type Axis = 'row' | 'column';

export interface ControlHost {
	view: EditorView;
	current: () => TableModel | null;
	inner: HTMLElement;
	drop: HTMLElement;
	rows: HTMLElement[];
	cells: HTMLElement[][];
	select: (axis: Axis, index: number) => void;
}

interface Drag {
	host: ControlHost;
	axis: Axis;
	from: number;
	track: HTMLElement[];
	button: HTMLElement;
}

function startDrag(drag: Drag, event: PointerEvent): void {
	const { host, axis, track, button } = drag;
	const { inner, drop } = host;
	const base = inner.getBoundingClientRect();
	const start = axis === 'row' ? event.clientY : event.clientX;
	let at = -1;
	let moved = false;

	const locate = (clientX: number, clientY: number): number => {
		let index = 0;
		for (const item of track) {
			const rect = item.getBoundingClientRect();
			const middle = axis === 'row' ? rect.top + rect.height / 2 : rect.left + rect.width / 2;
			if ((axis === 'row' ? clientY : clientX) > middle) index++;
		}
		return index;
	};

	const place = (index: number): void => {
		const last = track[track.length - 1].getBoundingClientRect();
		const rect = track[index]?.getBoundingClientRect() ?? last;
		if (axis === 'row')
			drop.style.top = `${(index < track.length ? rect.top : last.bottom) - base.top}px`;
		else drop.style.left = `${(index < track.length ? rect.left : last.right) - base.left}px`;
	};

	const move = (moveEvent: PointerEvent) => {
		const position = axis === 'row' ? moveEvent.clientY : moveEvent.clientX;
		if (!moved && Math.abs(position - start) < 4) return;
		moved = true;
		button.dataset.dragging = 'true';
		at = locate(moveEvent.clientX, moveEvent.clientY);
		drop.style.display = 'block';
		place(at);
	};

	const up = () => {
		window.removeEventListener('pointermove', move);
		window.removeEventListener('pointerup', up);
		drop.style.display = 'none';
		button.dataset.dragging = moved ? 'done' : '';
		if (!moved || at < 0) return;
		const table = host.current();
		if (!table) return;
		const target = at > drag.from ? at - 1 : at;
		if (axis === 'row') moveRowTo(host.view, table, drag.from, target);
		else moveColumnTo(host.view, table, drag.from, target);
	};

	window.addEventListener('pointermove', move);
	window.addEventListener('pointerup', up);
}

function control(label: string, cls: string, text = '+'): HTMLButtonElement {
	const button = document.createElement('button');
	button.type = 'button';
	button.className = cls;
	button.textContent = text;
	button.title = label;
	button.setAttribute('aria-label', label);
	button.addEventListener('mousedown', (event) => event.preventDefault());
	return button;
}

function handle(host: ControlHost, axis: Axis, index: number, track: HTMLElement[]): HTMLElement {
	const label = axis === 'row' ? m.editor_table_row_handle() : m.editor_table_column_handle();
	const button = control(label, `cm-lp-table-handle cm-lp-table-handle-${axis}`, '');
	button.addEventListener('click', (event) => {
		event.preventDefault();
		event.stopPropagation();
		if (button.dataset.dragging === 'done') button.dataset.dragging = '';
		else host.select(axis, index);
	});
	button.addEventListener('contextmenu', (event) => {
		event.preventDefault();
		event.stopPropagation();
		const table = host.current();
		if (!table) return;
		const at = { x: event.clientX, y: event.clientY };
		if (axis === 'row') openRowMenu(host.view, table.from, index, at);
		else openColumnMenu(host.view, table.from, index, at);
	});
	button.addEventListener('pointerdown', (event) => {
		if (event.button !== 0) return;
		event.preventDefault();
		event.stopPropagation();
		host.drop.dataset.axis = axis;
		startDrag({ host, axis, from: index, track, button }, event);
	});
	return button;
}

export function addControls(host: ControlHost): void {
	const { view, current, rows, cells } = host;
	rows.forEach((_, index) => cells[index][0].appendChild(handle(host, 'row', index, rows)));
	cells[0].forEach((cell, index) => cell.appendChild(handle(host, 'column', index, cells[0])));

	const addRow = control(m.editor_table_add_row(), 'cm-lp-table-add cm-lp-table-add-row');
	addRow.addEventListener('click', (event) => {
		event.preventDefault();
		event.stopPropagation();
		const table = current();
		if (table) appendRow(view, table);
	});
	const addCol = control(m.editor_table_add_column(), 'cm-lp-table-add cm-lp-table-add-col');
	addCol.addEventListener('click', (event) => {
		event.preventDefault();
		event.stopPropagation();
		const table = current();
		if (table) insertColumn(view, table, table.cols - 1, 'right');
	});
	host.inner.append(host.drop, addRow, addCol);
}
