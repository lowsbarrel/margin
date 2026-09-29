import { WidgetType, type EditorView } from '@codemirror/view';
import { resolveImage } from './assets';
import { openHref } from './click';
import { contextOf, staticPreview } from './context';
import { sourcesOf } from './decorate';
import { footnoteIndex } from './footnotes';
import {
	cellCoords,
	cellSession,
	endPress,
	focusEditor,
	mountCell,
	pressCell,
	unmountCell
} from './table-cell';
import { addControls } from './table-controls';
import { renderCell, type CellLookups } from './table-inline';
import { displayText, tableAt, type Align, type TableModel } from './table-model';
import { ensureCell } from './table-nav';
import {
	extendRange,
	inRange,
	sameRange,
	setCellRange,
	trackRange,
	type CellPoint,
	type CellRange
} from './table-range';

export interface ActiveCell {
	row: number;
	col: number;
}

const ALIGN_CSS: Record<Align, string> = {
	left: 'left',
	center: 'center',
	right: 'right',
	none: 'left'
};

const LINK = '[data-href], [data-wiki], [data-tag]';

class TableDom {
	readonly view: EditorView;
	readonly root = document.createElement('div');
	table: TableModel;
	private readonly cells: HTMLElement[][] = [];
	private readonly contents: HTMLElement[][] = [];
	private readonly painted: string[][] = [];
	private epoch = '';

	constructor(view: EditorView, table: TableModel) {
		this.view = view;
		this.table = table;
		this.build();
	}

	current(): TableModel | null {
		return tableAt(this.view.state, this.view.posAtDOM(this.root));
	}

	fits(table: TableModel): boolean {
		return table.rows.length === this.table.rows.length && table.cols === this.table.cols;
	}

	private editing(): { row: number; col: number } | null {
		const session = cellSession(this.view);
		return session && this.root.contains(session.host) ? session : null;
	}

	private locate(target: Element | null): CellPoint | null {
		const cell = target?.closest('.cm-lp-table-cell');
		if (!cell || !this.root.contains(cell)) return null;
		for (let row = 0; row < this.cells.length; row++) {
			const col = this.cells[row].indexOf(cell as HTMLElement);
			if (col >= 0) return { row, col };
		}
		return null;
	}

	private select(axis: 'row' | 'column', index: number): void {
		const table = this.current();
		if (!table) return;
		const anchor = axis === 'row' ? { row: index, col: 0 } : { row: 0, col: index };
		const head =
			axis === 'row'
				? { row: index, col: table.cols - 1 }
				: { row: table.rows.length - 1, col: index };
		const pos = ensureCell(this.view, { table, ...anchor });
		this.view.dispatch({ selection: { anchor: pos } });
		focusEditor(this.view);
		this.view.dispatch({ effects: setCellRange.of({ table: table.from, anchor, head }) });
	}

	private paintRange(range: CellRange | null): void {
		const on = (row: number, col: number) => inRange(range, row, col);
		this.cells.forEach((row, r) =>
			row.forEach((el, c) => {
				const selected = on(r, c);
				el.classList.toggle('cm-lp-table-cell-selected', selected);
				el.classList.toggle('cm-lp-sel-top', selected && !on(r - 1, c));
				el.classList.toggle('cm-lp-sel-bottom', selected && !on(r + 1, c));
				el.classList.toggle('cm-lp-sel-left', selected && !on(r, c - 1));
				el.classList.toggle('cm-lp-sel-right', selected && !on(r, c + 1));
			})
		);
		this.root.toggleAttribute('data-range', !!range);
	}

	paint(
		table: TableModel,
		active: ActiveCell | null,
		range: CellRange | null,
		epoch: string
	): void {
		this.table = table;
		if (epoch !== this.epoch) {
			this.epoch = epoch;
			for (const row of this.painted) row.fill('\u0000');
		}
		const editing = this.editing();
		table.rows.forEach((row, r) =>
			row.cells.forEach((cell, c) => {
				this.cells[r][c].style.textAlign = ALIGN_CSS[table.align[c] ?? 'none'];
				if (editing && editing.row === r && editing.col === c) return;
				this.render(r, c, cell.text);
			})
		);
		this.paintRange(range);
		if (editing && (!active || active.row !== editing.row || active.col !== editing.col)) {
			unmountCell(this.view);
			this.cells[editing.row][editing.col].classList.remove('cm-lp-table-cell-editing');
			this.render(editing.row, editing.col, table.rows[editing.row].cells[editing.col].text, true);
		}
		if (!active || (editing && active.row === editing.row && active.col === editing.col)) return;
		const cell = table.rows[active.row]?.cells[active.col];
		if (!cell) return;
		const host = this.contents[active.row][active.col];
		host.replaceChildren();
		this.painted[active.row][active.col] = '';
		this.cells[active.row][active.col].classList.add('cm-lp-table-cell-editing');
		mountCell(this.view, host, { ...active, from: cell.from, to: cell.to });
	}

	private render(row: number, col: number, text: string, force = false): void {
		if (!force && this.painted[row][col] === text) return;
		this.painted[row][col] = text;
		this.contents[row][col].replaceChildren(renderCell(displayText(text), this.lookups));
	}

	private readonly lookups: CellLookups = {
		image: (target, embed, label) =>
			resolveImage(target, embed, label, sourcesOf(contextOf(this.view.state))),
		footnote: (id) => footnoteIndex(this.view.state).numbers.get(id.toLowerCase())
	};

	destroy(): void {
		if (this.editing()) unmountCell(this.view);
	}

	private press(r: number, c: number, event: MouseEvent): void {
		const target = event.target as HTMLElement;
		if (target.closest('.cm-lp-table-handle, .cm-lp-table-add')) return;
		const { view } = this;
		if (view.state.facet(staticPreview)) return;
		const session = cellSession(view);
		const open = session?.host === this.contents[r][c];
		if (!open && target.closest(LINK)) return event.preventDefault();
		const now = this.current();
		if (event.shiftKey && event.button === 0 && now && session && this.editing() && !open) {
			event.preventDefault();
			return extendRange(view, now.from, session, { row: r, col: c });
		}
		if (now && event.button === 0) {
			const locate = (el: Element | null) => this.locate(el);
			trackRange(view, now.from, { row: r, col: c }, locate, () => endPress(view));
		}
		if (open && session.sub.dom.contains(target)) return;
		event.preventDefault();
		if (open) return pressCell(view, event);
		if (!now || event.button !== 0) return;
		const pos = ensureCell(view, { table: now, row: r, col: c });
		view.dispatch({ selection: { anchor: pos } });
		pressCell(view, event);
	}

	private open(r: number, c: number, event: MouseEvent): void {
		const link = (event.target as HTMLElement).closest<HTMLElement>(LINK);
		if (!link || cellSession(this.view)?.host === this.contents[r][c]) return;
		event.preventDefault();
		const ctx = contextOf(this.view.state);
		if (link.dataset.tag) ctx.openTag(link.dataset.tag);
		else if (link.dataset.wiki) ctx.openWikiLink(link.dataset.wiki);
		else if (link.dataset.href) openHref(link.dataset.href, ctx.vaultPath());
	}

	private build(): void {
		const { root } = this;
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
		const body = document.createElement('tbody');
		const rows: HTMLElement[] = [];

		this.table.rows.forEach((row, r) => {
			const tr = document.createElement('tr');
			this.cells.push([]);
			this.contents.push([]);
			this.painted.push([]);
			row.cells.forEach((_, c) => {
				const el = document.createElement(r === 0 ? 'th' : 'td');
				el.className = 'cm-lp-table-cell';
				const content = document.createElement('div');
				content.className = 'cm-lp-table-content';
				el.appendChild(content);
				el.addEventListener('mousedown', (event) => this.press(r, c, event));
				el.addEventListener('click', (event) => this.open(r, c, event));
				this.cells[r].push(el);
				this.contents[r].push(content);
				this.painted[r].push('\u0000');
				tr.appendChild(el);
			});
			rows.push(tr);
			(r === 0 ? head : body).appendChild(tr);
		});
		grid.append(head, body);
		inner.appendChild(grid);
		if (!this.view.state.facet(staticPreview))
			addControls({
				view: this.view,
				current: () => this.current(),
				inner,
				drop,
				rows,
				cells: this.cells,
				select: (axis, index) => this.select(axis, index)
			});
		scroll.appendChild(inner);
		root.appendChild(scroll);
	}
}

const doms = new WeakMap<HTMLElement, TableDom>();

export class TableWidget extends WidgetType {
	readonly table: TableModel;
	readonly source: string;
	readonly active: ActiveCell | null;
	readonly range: CellRange | null;
	readonly epoch: string;

	constructor(
		table: TableModel,
		source: string,
		active: ActiveCell | null,
		range: CellRange | null,
		epoch: string
	) {
		super();
		this.table = table;
		this.source = source;
		this.active = active;
		this.range = range;
		this.epoch = epoch;
	}

	eq(other: TableWidget): boolean {
		return (
			other.source === this.source &&
			other.active?.row === this.active?.row &&
			other.active?.col === this.active?.col &&
			sameRange(other.range, this.range) &&
			other.epoch === this.epoch
		);
	}

	updateDOM(dom: HTMLElement): boolean {
		const table = doms.get(dom);
		if (!table || !table.fits(this.table)) return false;
		table.paint(this.table, this.active, this.range, this.epoch);
		return true;
	}

	get estimatedHeight(): number {
		return this.table.rows.length * 34 + 36;
	}

	coordsAt(dom: HTMLElement, pos: number, side: number) {
		const table = doms.get(dom);
		const session = table && cellSession(table.view);
		if (!session || !dom.contains(session.host)) return null;
		return cellCoords(table.view, this.table.from + pos, side < 0 ? -1 : 1);
	}

	ignoreEvent(event: Event): boolean {
		// The editor context menu carries the table operations, so right-clicks must reach CodeMirror.
		return event.type !== 'contextmenu';
	}

	toDOM(view: EditorView): HTMLElement {
		const table = new TableDom(view, this.table);
		doms.set(table.root, table);
		table.paint(this.table, this.active, this.range, this.epoch);
		return table.root;
	}

	destroy(dom: HTMLElement): void {
		doms.get(dom)?.destroy();
	}
}
