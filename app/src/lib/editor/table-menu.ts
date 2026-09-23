// Floating menu for the table row/column grips. Built from plain DOM because it
// is opened by a ProseMirror plugin, which has no Svelte component to mount into.

import { computePosition, flip, offset, shift, type Placement } from '@floating-ui/dom';

import { getTableUiLayer, releaseTableUiLayer } from './table-ui-layer';

export interface TableMenuItem {
	label: string;
	onclick: () => void | Promise<void>;
	destructive?: boolean;
	disabled?: boolean;
}

export type TableMenuCloseReason = 'action' | 'escape' | 'outside';

interface OpenTableMenuOptions {
	anchor: HTMLElement | { x: number; y: number };
	label: string;
	items: TableMenuItem[];
	placement?: Placement;
	onclose?: (reason: TableMenuCloseReason) => void;
}

let closeCurrentMenu: ((reason: TableMenuCloseReason) => void) | null = null;

export function isTableMenuOpen(): boolean {
	return closeCurrentMenu !== null;
}

export function closeTableMenu(reason: TableMenuCloseReason = 'outside'): void {
	closeCurrentMenu?.(reason);
}

export function openTableMenu({
	anchor,
	label,
	items,
	placement = 'bottom-start',
	onclose
}: OpenTableMenuOptions): void {
	closeTableMenu('outside');

	const menu = document.createElement('div');
	menu.className = 'table-menu surface-popover';
	menu.dataset.tableMenu = 'true';
	menu.setAttribute('role', 'menu');
	menu.setAttribute('aria-label', label);
	menu.tabIndex = -1;

	// Keeping the default on mousedown would blur the editor before the item runs;
	// the table commands read `state.selection`, which must still be the grip's.
	menu.addEventListener('mousedown', (event) => event.preventDefault());

	const buttons: HTMLButtonElement[] = [];
	for (const item of items) {
		const button = document.createElement('button');
		button.type = 'button';
		button.className = 'table-menu-item';
		button.setAttribute('role', 'menuitem');
		button.disabled = Boolean(item.disabled);
		button.textContent = item.label;
		if (item.destructive) button.dataset.destructive = 'true';
		button.addEventListener('click', () => {
			close('action');
			void item.onclick();
		});
		menu.appendChild(button);
		buttons.push(button);
	}

	function onDocumentMouseDown(event: MouseEvent) {
		if (!menu.contains(event.target as Node)) close('outside');
	}

	function onDocumentKeyDown(event: KeyboardEvent) {
		if (event.key === 'Escape') close('escape');
		else if (event.key === 'Tab') close('outside');
	}

	function onMenuKeyDown(event: KeyboardEvent) {
		const enabled = buttons.filter((button) => !button.disabled);
		if (!enabled.length) return;
		const current = enabled.indexOf(document.activeElement as HTMLButtonElement);
		let next: number;
		if (event.key === 'ArrowDown') next = current + 1;
		else if (event.key === 'ArrowUp') next = current - 1;
		else if (event.key === 'Home') next = 0;
		else if (event.key === 'End') next = enabled.length - 1;
		else return;
		event.preventDefault();
		const count = enabled.length;
		enabled[((next % count) + count) % count].focus();
	}

	function close(reason: TableMenuCloseReason) {
		if (closeCurrentMenu !== close) return;
		closeCurrentMenu = null;
		document.removeEventListener('mousedown', onDocumentMouseDown, true);
		document.removeEventListener('keydown', onDocumentKeyDown, true);
		menu.removeEventListener('keydown', onMenuKeyDown);
		menu.remove();
		releaseTableUiLayer();
		onclose?.(reason);
	}

	menu.addEventListener('keydown', onMenuKeyDown);
	getTableUiLayer().appendChild(menu);
	closeCurrentMenu = close;
	document.addEventListener('mousedown', onDocumentMouseDown, true);
	document.addEventListener('keydown', onDocumentKeyDown, true);

	// A point anchor (click coordinates) is expressed as a zero-size rect; floating-ui
	// otherwise reads a live element's box.
	const reference: HTMLElement | { getBoundingClientRect: () => DOMRect } =
		anchor instanceof HTMLElement
			? anchor
			: {
					getBoundingClientRect: () =>
						({
							x: anchor.x,
							y: anchor.y,
							top: anchor.y,
							bottom: anchor.y,
							left: anchor.x,
							right: anchor.x,
							width: 0,
							height: 0
						}) as DOMRect
				};

	computePosition(reference, menu, {
		strategy: 'fixed',
		placement,
		middleware: [offset(6), flip({ padding: 8 }), shift({ padding: 8 })]
	}).then(({ x, y }) => {
		menu.style.left = `${x}px`;
		menu.style.top = `${y}px`;
	});

	const first = buttons.find((button) => !button.disabled);
	if (first) first.focus();
	else menu.focus();
}
