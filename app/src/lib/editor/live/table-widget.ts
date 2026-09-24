import { WidgetType, type EditorView } from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { displayText, tableAt, type Align, type TableModel } from './table-model';
import { openColumnMenu, openRowMenu } from './table-menu';
import { appendRow, insertColumn, moveColumnTo, moveRowTo } from './table-ops';
import { renderCell } from './table-inline';

const ALIGN_CSS: Record<Align, string> = {
	left: 'left',
	center: 'center',
	right: 'right',
	none: 'left'
};

interface DragOptions {
	view: EditorView;
	table: TableModel;
	axis: 'row' | 'column';
	from: number;
	track: HTMLElement[];
	inner: HTMLElement;
	drop: HTMLElement;
	button: HTMLElement;
}

function startDrag(options: DragOptions, event: PointerEvent): void {
	const { axis, track, inner, drop, button } = options;
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
		const table = tableAt(options.view.state, options.table.from);
		if (!table) return;
		const target = at > options.from ? at - 1 : at;
		if (axis === 'row') moveRowTo(options.view, table, options.from, target);
		else moveColumnTo(options.view, table, options.from, target);
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

function placeCaret(view: EditorView, table: TableModel, row: number, col: number): void {
	const current = tableAt(view.state, table.from) ?? table;
	const at = current.rows[Math.min(row, current.rows.length - 1)];
	const cell = at?.cells[Math.min(col, at.cells.length - 1)];
	if (!cell) return;
	view.focus();
	view.dispatch({ selection: { anchor: cell.from }, scrollIntoView: true });
}

function buildTable(view: EditorView, table: TableModel): HTMLElement {
	const root = document.createElement('div');
	root.className = 'cm-lp-table-block';
	root.contentEditable = 'false';
	const scroll = document.createElement('div');
	scroll.className = 'cm-lp-table-scroll';
	const inner = document.createElement('div');
	inner.className = 'cm-lp-table-inner';
	const drop = document.createElement('div');
	drop.className = 'cm-lp-table-drop';

	const grid = document.createElement('table');
	grid.className = 'cm-lp-table-grid';
	const head = document.createElement('thead');
	const headRow = document.createElement('tr');
	const body = document.createElement('tbody');
	const rowElements: HTMLElement[] = [headRow];

	const cellAt = (row: number, col: number, tag: string): HTMLElement => {
		const cell = table.rows[row].cells[col];
		const el = document.createElement(tag);
		el.className = 'cm-lp-table-cell';
		el.style.textAlign = ALIGN_CSS[table.align[col] ?? 'none'];
		el.appendChild(renderCell(displayText(cell.text)));
		el.addEventListener('mousedown', (event) => {
			if ((event.target as HTMLElement).closest('.cm-lp-table-handle, .cm-lp-table-add')) return;
			event.preventDefault();
			placeCaret(view, table, row, col);
		});
		return el;
	};

	const track = (
		axis: 'row' | 'column',
		index: number,
		el: HTMLElement,
		items: HTMLElement[]
	): HTMLElement => {
		const label = axis === 'row' ? m.editor_table_row_menu() : m.editor_table_column_menu();
		const button = control(label, `cm-lp-table-handle cm-lp-table-handle-${axis}`, '');
		button.addEventListener('click', (event) => {
			event.preventDefault();
			event.stopPropagation();
			if (button.dataset.dragging === 'done') {
				button.dataset.dragging = '';
				return;
			}
			const rect = button.getBoundingClientRect();
			const at = { x: rect.left, y: rect.bottom + 4 };
			if (axis === 'row') openRowMenu(view, table.from, index, at);
			else openColumnMenu(view, table.from, index, at);
		});
		button.addEventListener('pointerdown', (event) => {
			if (event.button !== 0) return;
			event.preventDefault();
			event.stopPropagation();
			drop.dataset.axis = axis;
			startDrag({ view, table, axis, from: index, track: items, inner, drop, button }, event);
		});
		el.appendChild(button);
		return el;
	};

	for (let col = 0; col < table.cols; col++) headRow.appendChild(cellAt(0, col, 'th'));
	head.appendChild(headRow);
	grid.appendChild(head);
	for (let row = 1; row < table.rows.length; row++) {
		const tr = document.createElement('tr');
		for (let col = 0; col < table.cols; col++) tr.appendChild(cellAt(row, col, 'td'));
		body.appendChild(tr);
		rowElements.push(tr);
	}
	grid.appendChild(body);

	const columnElements = Array.from(headRow.children) as HTMLElement[];
	rowElements.forEach((tr, index) => {
		const first = tr.firstElementChild;
		if (first) track('row', index, first as HTMLElement, rowElements);
	});
	columnElements.forEach((cell, index) => track('column', index, cell, columnElements));

	const addRow = control(m.editor_table_add_row(), 'cm-lp-table-add cm-lp-table-add-row');
	addRow.addEventListener('click', (event) => {
		event.preventDefault();
		event.stopPropagation();
		const current = tableAt(view.state, table.from);
		if (current) appendRow(view, current);
	});
	const addCol = control(m.editor_table_add_column(), 'cm-lp-table-add cm-lp-table-add-col');
	addCol.addEventListener('click', (event) => {
		event.preventDefault();
		event.stopPropagation();
		const current = tableAt(view.state, table.from);
		if (current) insertColumn(view, current, current.cols - 1, 'right');
	});

	inner.append(grid, drop, addRow, addCol);
	scroll.appendChild(inner);
	root.appendChild(scroll);
	return root;
}

export class TableWidget extends WidgetType {
	readonly table: TableModel;
	readonly source: string;

	constructor(table: TableModel, source: string) {
		super();
		this.table = table;
		this.source = source;
	}

	eq(other: TableWidget): boolean {
		return other.source === this.source;
	}

	ignoreEvent(): boolean {
		return true;
	}

	toDOM(view: EditorView): HTMLElement {
		return buildTable(view, this.table);
	}
}
