/**
 * Table controls — hover affordances to append a row/column, row and column
 * grips that select and open a menu, and pointer-based reordering (native DnD
 * does not work in the Tauri WebView). Adapted from Docmost (Apache 2.0).
 *
 * Everything floats in a fixed body-level layer: the editor pane clips its own
 * overflow, so handles pinned to `editor.options.element` were cut near edges.
 */

import type { Editor } from '@tiptap/core';
import { Extension } from '@tiptap/core';
import { Plugin, PluginKey, TextSelection } from '@tiptap/pm/state';
import { CellSelection, TableMap, cellAround } from '@tiptap/pm/tables';
import type { EditorView } from '@tiptap/pm/view';
import { computePosition, offset, type Placement } from '@floating-ui/dom';
import * as m from '$lib/paraglide/messages.js';

import { getTableUiLayer, releaseTableUiLayer } from './table-ui-layer';
import { closeTableMenu, isTableMenuOpen, openTableMenu, type TableMenuItem } from './table-menu';
import {
	type DraggingDOMs,
	type HoveringCellInfo,
	type TablePosition,
	findTable,
	getCellInfoAt,
	getDndRelatedDOMs,
	getDragOverColumn,
	getDragOverRow,
	getHoveringCell,
	getSelectionRangeInColumn,
	getSelectionRangeInRow,
	moveColumn,
	moveRow
} from './table-dnd-utils';

// ─── Constants ──────────────────────────────────────────────────────────

const DROP_INDICATOR_WIDTH = 2;
const EDGE_THRESHOLD = 80;
const SCROLL_SPEED = 12;
const DRAG_THRESHOLD = 4; // px before mousedown becomes a drag
// Half of the `table-drag-handle` box in editor-styles.css: centres the grip on
// the edge it marks.
const HANDLE_OFFSET = -9;
// Floating-ui shifts a floating element against the sign of its main-axis offset,
// so this is +24: away from the cell, clearing the whole-block drag handle that
// content-drag puts 24px left of a block. Overlapping it would make the block
// handle unreachable while the pointer is over a table's first row.
const ROW_HANDLE_OFFSET = 24;
const HANDLE_GAP = 4; // px between the table edge and its append button
const HIDE_DELAY = 140; // ms of grace while the pointer crosses from table to handle

// ─── Extension export ───────────────────────────────────────────────────

const TableDndKey = new PluginKey('table-drag-and-drop');

interface TableControls {
	onPointerMove(event: PointerEvent): void;
	onPointerLeave(): void;
	destroy(): void;
}

/** Nearest scrollable ancestor, or the document scroller when nothing scrolls. */
function findScrollParent(start: HTMLElement): HTMLElement | null {
	let el: HTMLElement | null = start.parentElement;
	while (el) {
		const { overflowY } = getComputedStyle(el);
		if (/auto|scroll/.test(overflowY) && el.scrollHeight > el.clientHeight + 1) return el;
		el = el.parentElement;
	}
	const root = document.scrollingElement;
	return root instanceof HTMLElement ? root : null;
}

function createTableControls(view: EditorView, editor: Editor): TableControls {
	const layer = getTableUiLayer();
	const scrollParent = findScrollParent(view.dom);

	// ── DOM elements ──────────────────────────────────

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

	const colHandle = createHandle('col');
	const rowHandle = createHandle('row');

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

	const addRowHandle = createAddHandle('row');
	const addColumnHandle = createAddHandle('col');

	const preview = document.createElement('div');
	preview.className = 'table-dnd-preview ProseMirror';

	const dropIndicator = document.createElement('div');
	dropIndicator.className = 'table-dnd-drop-indicator';
	dropIndicator.dataset.dragging = 'false';

	layer.append(colHandle, rowHandle, addRowHandle, addColumnHandle, preview, dropIndicator);

	// ── State ─────────────────────────────────────────

	let hoveringCell: HoveringCellInfo | undefined;
	let dragging = false;
	let draggingIndex = -1;
	let droppingIndex = -1;
	let dragCellPos: number | undefined;
	let startCoords = { x: 0, y: 0 };
	let scrollInterval: number | undefined;
	let hideTimer: number | undefined;
	let hoverRaf = 0;
	let latestPointer: PointerEvent | null = null;
	let stopDragListeners: (() => void) | null = null;
	let menuOpen = false;
	const pointerOverHandle = new Set<HTMLElement>();

	// ── Positioning ───────────────────────────────────

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

	// ── Visibility ────────────────────────────────────

	function cancelHide() {
		if (hideTimer !== undefined) {
			window.clearTimeout(hideTimer);
			hideTimer = undefined;
		}
	}

	function hideHandles() {
		pointerOverHandle.clear();
		colHandle.classList.remove('is-visible');
		rowHandle.classList.remove('is-visible');
		addRowHandle.classList.remove('is-visible');
		addColumnHandle.classList.remove('is-visible');
		hoveringCell = undefined;
	}

	// Leaving the table to reach a handle must not hide it mid-journey, so the
	// teardown is deferred and a hovered (or focused) handle cancels it.
	function scheduleHide() {
		cancelHide();
		hideTimer = window.setTimeout(() => {
			hideTimer = undefined;
			if (dragging || pointerOverHandle.size > 0 || isTableMenuOpen()) return;
			if (layer.contains(document.activeElement)) return;
			hideHandles();
		}, HIDE_DELAY);
	}

	for (const el of [colHandle, rowHandle, addRowHandle, addColumnHandle]) {
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

	function showHandles(cell: HoveringCellInfo) {
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

	/** Re-anchor the append buttons after the table changed size under them. */
	function repositionAddHandles(tablePos: number) {
		const dom = view.nodeDOM(tablePos);
		if (!(dom instanceof HTMLElement)) return;
		const table = dom.matches('table') ? dom : dom.querySelector('table');
		if (table instanceof HTMLTableElement) showAddHandles(table);
	}

	// ── Preview ───────────────────────────────────────

	function showPreview(doms: DraggingDOMs, index: number, type: 'col' | 'row') {
		while (preview.firstChild) preview.removeChild(preview.firstChild);

		const tRect = doms.table.getBoundingClientRect();
		const cRect = doms.cell.getBoundingClientRect();

		if (type === 'col') {
			Object.assign(preview.style, {
				display: 'block',
				width: `${cRect.width}px`,
				height: `${tRect.height}px`
			});
		} else {
			Object.assign(preview.style, {
				display: 'block',
				width: `${tRect.width}px`,
				height: `${cRect.height}px`
			});
		}

		const previewTable = document.createElement('table');
		const body = document.createElement('tbody');
		previewTable.appendChild(body);
		preview.appendChild(previewTable);
		const rows = doms.table.querySelectorAll('tr');

		if (type === 'row') {
			const row = rows[index];
			if (row) body.appendChild(row.cloneNode(true));
		} else {
			rows.forEach((row) => {
				const rowDOM = row.cloneNode(false) as HTMLElement;
				const cells = row.querySelectorAll('th,td');
				if (cells[index]) {
					rowDOM.appendChild(cells[index].cloneNode(true));
					body.appendChild(rowDOM);
				}
			});
		}

		computePosition(doms.cell, preview, {
			strategy: 'fixed',
			placement: type === 'row' ? 'right' : 'bottom',
			middleware: [
				offset(({ rects }) => (type === 'col' ? -rects.reference.height : -rects.reference.width))
			]
		}).then(({ x, y }) => {
			Object.assign(preview.style, { left: `${x}px`, top: `${y}px` });
		});
	}

	function updatePreviewPosition(x: number, y: number, cell: HTMLElement, type: 'col' | 'row') {
		const vEl = {
			contextElement: cell,
			getBoundingClientRect: () => {
				const r = cell.getBoundingClientRect();
				return {
					width: r.width,
					height: r.height,
					right: x + r.width / 2,
					bottom: y + r.height / 2,
					top: y - r.height / 2,
					left: x - r.width / 2,
					x: x - r.width / 2,
					y: y - r.height / 2
				};
			}
		};
		computePosition(vEl, preview, {
			strategy: 'fixed',
			placement: type === 'row' ? 'right' : 'bottom'
		}).then(({ x: px, y: py }) => {
			if (type === 'row') Object.assign(preview.style, { top: `${py}px` });
			else Object.assign(preview.style, { left: `${px}px` });
		});
	}

	function hidePreview() {
		while (preview.firstChild) preview.removeChild(preview.firstChild);
		Object.assign(preview.style, { display: 'none' });
	}

	// ── Drop indicator ────────────────────────────────

	function showDropIndicator(doms: DraggingDOMs, type: 'col' | 'row') {
		const tRect = doms.table.getBoundingClientRect();
		if (type === 'col') {
			Object.assign(dropIndicator.style, {
				display: 'block',
				width: `${DROP_INDICATOR_WIDTH}px`,
				height: `${tRect.height}px`
			});
		} else {
			Object.assign(dropIndicator.style, {
				display: 'block',
				width: `${tRect.width}px`,
				height: `${DROP_INDICATOR_WIDTH}px`
			});
		}
		computePosition(doms.cell, dropIndicator, {
			strategy: 'fixed',
			placement: type === 'row' ? 'right' : 'bottom',
			middleware: [
				offset(({ rects }) => (type === 'col' ? -rects.reference.height : -rects.reference.width))
			]
		}).then(({ x, y }) => {
			Object.assign(dropIndicator.style, { left: `${x}px`, top: `${y}px` });
		});
		dropIndicator.dataset.dragging = 'true';
	}

	function updateDropIndicator(target: Element, direction: string, type: 'col' | 'row') {
		if (type === 'col') {
			computePosition(target, dropIndicator, {
				strategy: 'fixed',
				placement: direction === 'left' ? 'left' : 'right',
				middleware: [offset(direction === 'left' ? -DROP_INDICATOR_WIDTH : 0)]
			}).then(({ x }) => {
				Object.assign(dropIndicator.style, { left: `${x}px` });
			});
		} else {
			computePosition(target, dropIndicator, {
				strategy: 'fixed',
				placement: direction === 'up' ? 'top' : 'bottom',
				middleware: [offset(direction === 'up' ? -DROP_INDICATOR_WIDTH : 0)]
			}).then(({ y }) => {
				Object.assign(dropIndicator.style, { top: `${y}px` });
			});
		}
	}

	function hideDropIndicator() {
		Object.assign(dropIndicator.style, { display: 'none' });
		dropIndicator.dataset.dragging = 'false';
	}

	// ── Auto-scroll ───────────────────────────────────

	function stopAutoScroll() {
		if (scrollInterval) {
			clearInterval(scrollInterval);
			scrollInterval = undefined;
		}
	}

	function checkAutoScroll(clientX: number, clientY: number, doms?: DraggingDOMs) {
		stopAutoScroll();
		if (scrollParent) {
			const rect = scrollParent.getBoundingClientRect();
			if (clientY < rect.top + EDGE_THRESHOLD) {
				scrollInterval = window.setInterval(() => {
					scrollParent.scrollTop = Math.max(0, scrollParent.scrollTop - SCROLL_SPEED);
				}, 16);
				return;
			}
			if (clientY > rect.bottom - EDGE_THRESHOLD) {
				scrollInterval = window.setInterval(() => {
					scrollParent.scrollTop = Math.min(
						scrollParent.scrollHeight,
						scrollParent.scrollTop + SCROLL_SPEED
					);
				}, 16);
				return;
			}
		}
		if (doms) {
			const wrapper = doms.table.closest<HTMLElement>('.tableWrapper');
			if (wrapper) {
				const rect = wrapper.getBoundingClientRect();
				if (clientX < rect.left + EDGE_THRESHOLD) {
					scrollInterval = window.setInterval(() => {
						wrapper.scrollLeft -= SCROLL_SPEED;
					}, 16);
					return;
				}
				if (clientX > rect.right - EDGE_THRESHOLD) {
					scrollInterval = window.setInterval(() => {
						wrapper.scrollLeft += SCROLL_SPEED;
					}, 16);
				}
			}
		}
	}

	// ── Commands ──────────────────────────────────────

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

	function moveGrip(kind: 'col' | 'row', cell: HoveringCellInfo, target: number) {
		const tr = editor.state.tr;
		const moved =
			kind === 'col'
				? moveColumn({
						tr,
						originIndex: cell.colIndex,
						targetIndex: target,
						select: true,
						pos: cell.cellPos
					})
				: moveRow({
						tr,
						originIndex: cell.rowIndex,
						targetIndex: target,
						select: true,
						pos: cell.cellPos
					});
		if (!moved) return;
		view.dispatch(tr);
		view.focus();
	}

	function openGripMenu(kind: 'col' | 'row', cell: HoveringCellInfo) {
		const table = findTable(view.state.doc.resolve(cell.cellPos));
		if (!table) return;
		if (!selectGrip(kind, cell)) return;
		menuOpen = true;
		openTableMenu({
			anchor: kind === 'row' ? rowHandle : colHandle,
			placement: kind === 'row' ? 'right-start' : 'bottom-start',
			label: kind === 'row' ? m.editor_table_row_menu() : m.editor_table_column_menu(),
			items: gripperMenuItems(kind, cell, TableMap.get(table.node)),
			onclose: (reason) => {
				menuOpen = false;
				hideHandles();
				if (reason === 'escape') view.focus();
			}
		});
	}

	/** The grip a keyboard user is on acts on the cell their caret sits in. */
	function selectionCell(): HoveringCellInfo | undefined {
		const selection = view.state.selection;
		const $cell =
			selection instanceof CellSelection ? selection.$anchorCell : cellAround(selection.$from);
		return $cell ? getCellInfoAt($cell) : undefined;
	}

	function openGripMenuFromSelection(kind: 'col' | 'row') {
		const cell = selectionCell();
		if (cell) openGripMenu(kind, cell);
	}

	/** The append buttons act on the table under the pointer, or failing that on
	 *  the one the caret sits in (the pointer may never cross the editor between
	 *  two clicks on the handles). */
	function tableForAppend(): TablePosition | undefined {
		const cell = hoveringCell ?? selectionCell();
		if (!cell) return;
		return findTable(view.state.doc.resolve(cell.cellPos));
	}

	// Appends must start from a caret inside the target cell: the table commands
	// only act on a cell selection, and `addRowAfter` on the last row extends the
	// table by one row.
	function appendRow() {
		const table = tableForAppend();
		if (!table) return;
		const map = TableMap.get(table.node);
		const target = table.start + map.positionAt(map.height - 1, 0, table.node);
		const tr = view.state.tr.setSelection(TextSelection.near(view.state.doc.resolve(target + 1)));
		view.dispatch(tr);
		editor.chain().focus().addRowAfter().run();
		hideHandlesAfterAppend(table.pos);
	}

	function appendColumn() {
		const table = tableForAppend();
		if (!table) return;
		const map = TableMap.get(table.node);
		const target = table.start + map.positionAt(0, map.width - 1, table.node);
		const tr = view.state.tr.setSelection(TextSelection.near(view.state.doc.resolve(target + 1)));
		view.dispatch(tr);
		editor.chain().focus().addColumnAfter().run();
		hideHandlesAfterAppend(table.pos);
	}

	function hideHandlesAfterAppend(tablePos: number) {
		colHandle.classList.remove('is-visible');
		rowHandle.classList.remove('is-visible');
		repositionAddHandles(tablePos);
	}

	// ── Mouse drag logic ──────────────────────────────

	function onGripMouseDown(kind: 'col' | 'row', event: MouseEvent) {
		event.preventDefault();
		event.stopPropagation();

		const savedCell = hoveringCell;
		if (!savedCell) return;

		const initX = event.clientX;
		const initY = event.clientY;
		let started = false;

		dragCellPos = savedCell.cellPos;
		draggingIndex = kind === 'col' ? savedCell.colIndex : savedCell.rowIndex;
		droppingIndex = -1;

		const doms = getDndRelatedDOMs(view, savedCell.cellPos, draggingIndex, kind);

		const onMouseMove = (ev: MouseEvent) => {
			const dx = ev.clientX - initX;
			const dy = ev.clientY - initY;

			if (!started) {
				if (Math.abs(dx) + Math.abs(dy) < DRAG_THRESHOLD) return;
				started = true;
				dragging = true;
				startCoords = { x: initX, y: initY };
				document.body.style.cursor = 'grabbing';
				document.body.style.userSelect = 'none';

				if (doms) {
					const idx = kind === 'col' ? savedCell.colIndex : savedCell.rowIndex;
					showPreview(doms, idx, kind);
					showDropIndicator(doms, kind);
				}
			}

			if (!doms) return;

			updatePreviewPosition(ev.clientX, ev.clientY, doms.cell, kind);
			checkAutoScroll(ev.clientX, ev.clientY, doms);

			if (kind === 'col') {
				const dir = startCoords.x > ev.clientX ? 'left' : 'right';
				const over = getDragOverColumn(doms.table, ev.clientX);
				if (over) {
					droppingIndex = over[1];
					updateDropIndicator(over[0], dir, 'col');
				}
			} else {
				const dir = startCoords.y > ev.clientY ? 'up' : 'down';
				const over = getDragOverRow(doms.table, ev.clientY);
				if (over) {
					droppingIndex = over[1];
					updateDropIndicator(over[0], dir, 'row');
				}
			}
		};

		const cleanup = () => {
			stopDragListeners = null;
			window.removeEventListener('mousemove', onMouseMove, true);
			window.removeEventListener('mouseup', onMouseUp, true);
			document.body.style.cursor = '';
			document.body.style.userSelect = '';
			stopAutoScroll();
			hidePreview();
			hideDropIndicator();
			dragging = false;
			draggingIndex = -1;
			droppingIndex = -1;
			dragCellPos = undefined;
		};

		const onMouseUp = () => {
			const wasDrag = started && dragging && dragCellPos != null && droppingIndex >= 0;
			const dragPos = dragCellPos;
			const origin = draggingIndex;
			const target = droppingIndex;
			cleanup();

			if (wasDrag && dragPos != null) {
				const tr = editor.state.tr;
				const moved =
					kind === 'col'
						? moveColumn({
								tr,
								originIndex: origin,
								targetIndex: target,
								select: true,
								pos: dragPos
							})
						: moveRow({ tr, originIndex: origin, targetIndex: target, select: true, pos: dragPos });
				if (moved) view.dispatch(tr);
				return;
			}
			if (!started) openGripMenu(kind, savedCell);
		};

		stopDragListeners = cleanup;
		window.addEventListener('mousemove', onMouseMove, true);
		window.addEventListener('mouseup', onMouseUp, true);
	}

	// ── Affordance listeners ──────────────────────────

	const onColDown = (e: MouseEvent) => onGripMouseDown('col', e);
	const onRowDown = (e: MouseEvent) => onGripMouseDown('row', e);
	colHandle.addEventListener('mousedown', onColDown);
	rowHandle.addEventListener('mousedown', onRowDown);

	// A mouse click is already handled on mouseup (drag or open); `detail === 0`
	// is the keyboard activation of an already focused grip.
	const onColClick = (e: MouseEvent) => {
		if (e.detail === 0) openGripMenuFromSelection('col');
	};
	const onRowClick = (e: MouseEvent) => {
		if (e.detail === 0) openGripMenuFromSelection('row');
	};
	colHandle.addEventListener('click', onColClick);
	rowHandle.addEventListener('click', onRowClick);

	addRowHandle.addEventListener('click', appendRow);
	addColumnHandle.addEventListener('click', appendColumn);

	// ── Hover tracking ────────────────────────────────

	function updateHover(event: PointerEvent) {
		const target = event.target as HTMLElement | null;
		const table = target?.closest?.('table');
		if (!table || !view.dom.contains(table)) {
			scheduleHide();
			return;
		}
		const cell = getHoveringCell(view, event);
		if (!cell) {
			scheduleHide();
			return;
		}
		cancelHide();
		hoveringCell = cell;
		showHandles(cell);
		showAddHandles(table as HTMLTableElement);
	}

	function onPointerMove(event: PointerEvent) {
		if (dragging) return;
		latestPointer = event;
		if (hoverRaf) return;
		hoverRaf = requestAnimationFrame(() => {
			hoverRaf = 0;
			const pending = latestPointer;
			latestPointer = null;
			if (pending && !dragging) updateHover(pending);
		});
	}

	function onPointerLeave() {
		latestPointer = null;
		scheduleHide();
	}

	function destroy() {
		cancelHide();
		if (hoverRaf) cancelAnimationFrame(hoverRaf);
		stopDragListeners?.();
		if (menuOpen) closeTableMenu();
		colHandle.removeEventListener('mousedown', onColDown);
		rowHandle.removeEventListener('mousedown', onRowDown);
		colHandle.removeEventListener('click', onColClick);
		rowHandle.removeEventListener('click', onRowClick);
		addRowHandle.removeEventListener('click', appendRow);
		addColumnHandle.removeEventListener('click', appendColumn);
		for (const el of [colHandle, rowHandle, addRowHandle, addColumnHandle]) el.remove();
		preview.remove();
		dropIndicator.remove();
		releaseTableUiLayer();
	}

	return {
		onPointerMove,
		onPointerLeave,
		destroy
	};
}

export const TableDndExtension = Extension.create({
	name: 'tableDragAndDrop',
	addProseMirrorPlugins() {
		const editor = this.editor;
		let controls: TableControls | null = null;

		return [
			new Plugin({
				key: TableDndKey,

				view(view) {
					controls = createTableControls(view, editor);
					return {
						destroy() {
							controls?.destroy();
							controls = null;
						}
					};
				},

				props: {
					handleDOMEvents: {
						pointermove: (_view, event) => {
							if (editor.isEditable) controls?.onPointerMove(event as PointerEvent);
							return false;
						},
						pointerleave: () => {
							controls?.onPointerLeave();
							return false;
						}
					}
				}
			})
		];
	}
});
