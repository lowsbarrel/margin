import { computePosition, offset } from '@floating-ui/dom';

import { getTableUiLayer, releaseTableUiLayer } from '../table-ui-layer';
import type { DraggingDOMs } from './table-hit-test';

const DROP_INDICATOR_WIDTH = 2;

export interface TableDragVisuals {
	showPreview(doms: DraggingDOMs, index: number, type: 'col' | 'row'): void;
	updatePreviewPosition(x: number, y: number, cell: HTMLElement, type: 'col' | 'row'): void;
	hidePreview(): void;
	showDropIndicator(doms: DraggingDOMs, type: 'col' | 'row'): void;
	updateDropIndicator(target: Element, direction: string, type: 'col' | 'row'): void;
	hideDropIndicator(): void;
	destroy(): void;
}

export function createTableDragVisuals(): TableDragVisuals {
	const layer = getTableUiLayer();

	const preview = document.createElement('div');
	preview.className = 'table-dnd-preview ProseMirror';

	const dropIndicator = document.createElement('div');
	dropIndicator.className = 'table-dnd-drop-indicator';
	dropIndicator.dataset.dragging = 'false';

	layer.append(preview, dropIndicator);

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

	function destroy() {
		preview.remove();
		dropIndicator.remove();
		releaseTableUiLayer();
	}

	return {
		showPreview,
		updatePreviewPosition,
		hidePreview,
		showDropIndicator,
		updateDropIndicator,
		hideDropIndicator,
		destroy
	};
}
