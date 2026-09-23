import type { Editor } from '@tiptap/core';
import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import type { EditorView } from '@tiptap/pm/view';

import { closeTableMenu, isTableMenuOpen } from './table-menu';
import { createTableAutoScroll } from './table/table-auto-scroll';
import { createTableCommands } from './table/table-commands';
import { createTableDragVisuals } from './table/table-drag-visuals';
import { createTableHandles } from './table/table-handles';
import {
	getDndRelatedDOMs,
	getDragOverColumn,
	getDragOverRow,
	getHoveringCell
} from './table/table-hit-test';
import type { HoveringCellInfo } from './table/table-query';

const DRAG_THRESHOLD = 4;
const TableDndKey = new PluginKey('table-drag-and-drop');

interface TableControls {
	onPointerMove(event: PointerEvent): void;
	onPointerLeave(): void;
	destroy(): void;
}

function createTableControls(view: EditorView, editor: Editor): TableControls {
	let hoveringCell: HoveringCellInfo | undefined;
	let dragging = false;
	let draggingIndex = -1;
	let droppingIndex = -1;
	let dragCellPos: number | undefined;
	let startCoords = { x: 0, y: 0 };
	let hoverRaf = 0;
	let latestPointer: PointerEvent | null = null;
	let stopDragListeners: (() => void) | null = null;
	let menuOpen = false;

	const handles = createTableHandles(view, () => dragging || isTableMenuOpen());
	const visuals = createTableDragVisuals();
	const autoScroll = createTableAutoScroll(view.dom);
	const commands = createTableCommands({
		view,
		editor,
		handles,
		hoveringCell: () => hoveringCell,
		onMenuOpenChange: (open) => {
			menuOpen = open;
		},
		onMenuClose: (reason) => {
			hideHandles();
			if (reason === 'escape') view.focus();
		}
	});

	function hideHandles() {
		handles.hide();
		hoveringCell = undefined;
	}

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
					visuals.showPreview(doms, idx, kind);
					visuals.showDropIndicator(doms, kind);
				}
			}

			if (!doms) return;

			visuals.updatePreviewPosition(ev.clientX, ev.clientY, doms.cell, kind);
			autoScroll.check(ev.clientX, ev.clientY, doms);

			if (kind === 'col') {
				const dir = startCoords.x > ev.clientX ? 'left' : 'right';
				const over = getDragOverColumn(doms.table, ev.clientX);
				if (over) {
					droppingIndex = over[1];
					visuals.updateDropIndicator(over[0], dir, 'col');
				}
			} else {
				const dir = startCoords.y > ev.clientY ? 'up' : 'down';
				const over = getDragOverRow(doms.table, ev.clientY);
				if (over) {
					droppingIndex = over[1];
					visuals.updateDropIndicator(over[0], dir, 'row');
				}
			}
		};

		const cleanup = () => {
			stopDragListeners = null;
			window.removeEventListener('mousemove', onMouseMove, true);
			window.removeEventListener('mouseup', onMouseUp, true);
			document.body.style.cursor = '';
			document.body.style.userSelect = '';
			autoScroll.stop();
			visuals.hidePreview();
			visuals.hideDropIndicator();
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
				commands.applyMove(kind, dragPos, origin, target);
				return;
			}
			if (!started) commands.openMenu(kind, savedCell);
		};

		stopDragListeners = cleanup;
		window.addEventListener('mousemove', onMouseMove, true);
		window.addEventListener('mouseup', onMouseUp, true);
	}

	const onColDown = (e: MouseEvent) => onGripMouseDown('col', e);
	const onRowDown = (e: MouseEvent) => onGripMouseDown('row', e);
	handles.colHandle.addEventListener('mousedown', onColDown);
	handles.rowHandle.addEventListener('mousedown', onRowDown);

	// A click is already handled on mouseup (drag or menu); `detail === 0` is the keyboard activation of a focused grip.
	const onColClick = (e: MouseEvent) => {
		if (e.detail === 0) commands.openMenuFromSelection('col');
	};
	const onRowClick = (e: MouseEvent) => {
		if (e.detail === 0) commands.openMenuFromSelection('row');
	};
	handles.colHandle.addEventListener('click', onColClick);
	handles.rowHandle.addEventListener('click', onRowClick);

	handles.addRowHandle.addEventListener('click', commands.appendRow);
	handles.addColumnHandle.addEventListener('click', commands.appendColumn);

	function updateHover(event: PointerEvent) {
		const target = event.target as HTMLElement | null;
		const table = target?.closest?.('table');
		if (!table || !view.dom.contains(table)) {
			handles.scheduleHide();
			return;
		}
		const cell = getHoveringCell(view, event);
		if (!cell) {
			handles.scheduleHide();
			return;
		}
		handles.cancelHide();
		hoveringCell = cell;
		handles.showForCell(cell);
		handles.showAddHandles(table as HTMLTableElement);
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
		handles.scheduleHide();
	}

	function destroy() {
		handles.cancelHide();
		if (hoverRaf) cancelAnimationFrame(hoverRaf);
		stopDragListeners?.();
		if (menuOpen) closeTableMenu();
		handles.colHandle.removeEventListener('mousedown', onColDown);
		handles.rowHandle.removeEventListener('mousedown', onRowDown);
		handles.colHandle.removeEventListener('click', onColClick);
		handles.rowHandle.removeEventListener('click', onRowClick);
		handles.addRowHandle.removeEventListener('click', commands.appendRow);
		handles.addColumnHandle.removeEventListener('click', commands.appendColumn);
		handles.destroy();
		visuals.destroy();
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
