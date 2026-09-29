import { panes, fileTitle } from '$lib/stores/panes.svelte';
import type { Tab } from '$lib/stores/panes.svelte';
import { drag } from '$lib/stores/drag.svelte';
import type { DragItem, StripTarget } from '$lib/stores/drag.svelte';
import { clampInsertIndex, reorderTabGap } from '$lib/stores/pane-layout';
import { startPointerDrag } from '$lib/utils/drag-handler';

export type PaneZoneMode = 'none' | 'split' | 'all';

export function paneZoneMode(
	item: DragItem | null,
	paneIndex: number,
	tabCount: number
): PaneZoneMode {
	if (!item) return 'none';
	if (item.kind === 'file') return item.isDir ? 'none' : 'split';
	if (item.paneIndex !== paneIndex) return 'all';
	return tabCount >= 2 ? 'split' : 'none';
}

export function tabStripAccepts(
	item: DragItem | null,
	paneIndex: number,
	tabCount: number
): boolean {
	if (!item) return false;
	if (item.kind === 'file') return !item.isDir;
	return item.paneIndex !== paneIndex || tabCount >= 2;
}

export function stripGapAt(tabsEl: HTMLElement, clientX: number): number {
	let gap = 0;
	for (const el of tabsEl.querySelectorAll<HTMLElement>('[data-tab]')) {
		const rect = el.getBoundingClientRect();
		if (clientX < rect.left + rect.width / 2) return gap;
		gap++;
	}
	return gap;
}

export function stripDropGap(
	item: DragItem | null,
	paneIndex: number,
	tabs: Tab[],
	gap: number
): number {
	if (!item) return gap;
	if (item.kind === 'tab' && item.paneIndex === paneIndex)
		return reorderTabGap(tabs, item.tabIndex, gap);
	return clampInsertIndex(tabs, item.kind === 'tab' ? item.pinned : false, gap);
}

export function handleTabMouseDown(e: MouseEvent, paneIndex: number, tabIndex: number) {
	if (e.button !== 0) return;
	if (panes.list.length === 1 && panes.list[0].tabs.length === 1) return;
	e.preventDefault();
	const target = panes.list[paneIndex].tabs[tabIndex];
	const label = fileTitle(target.path);

	startPointerDrag(
		e,
		{ kind: 'tab', paneIndex, tabIndex, label, pinned: target.pinned },
		undefined,
		() => panes.switchTab(paneIndex, tabIndex)
	);
}

export async function executeDrop(target: {
	paneIndex: number;
	zone: 'left' | 'center' | 'right';
}) {
	const item = drag.item;
	if (!item) return;
	if (item.kind === 'file') {
		if (item.isDir) return;
		if (target.zone === 'center') {
			if (target.paneIndex !== panes.activePaneIndex) await panes.focusPane(target.paneIndex);
			await panes.openFile(item.path);
		} else {
			await panes.openFileInNewPane(item.path, target.paneIndex, target.zone);
		}
	} else if (item.kind === 'tab') {
		const { paneIndex: srcPane, tabIndex: srcTab } = item;
		if (target.zone === 'center') await panes.moveTabToPane(srcPane, srcTab, target.paneIndex);
		else await panes.moveTabToNewPane(srcPane, srcTab, target.paneIndex, target.zone);
	}
}

export async function executeStripDrop(target: StripTarget) {
	const item = drag.item;
	if (!item) return;
	if (item.kind === 'file') {
		if (item.isDir) return;
		if (target.paneIndex !== panes.activePaneIndex) await panes.focusPane(target.paneIndex);
		await panes.openFile(item.path, target.paneIndex, target.index);
		return;
	}
	if (item.paneIndex === target.paneIndex) {
		panes.moveTabWithinPane(target.paneIndex, item.tabIndex, target.index);
		return;
	}
	await panes.moveTabToPane(item.paneIndex, item.tabIndex, target.paneIndex, target.index);
}

export function startDividerDrag(
	e: MouseEvent,
	dividerIndex: number,
	panesContainerEl: HTMLElement,
	setResizing: (v: boolean) => void
) {
	e.preventDefault();
	setResizing(true);
	const startX = e.clientX;
	const containerWidth = panesContainerEl.getBoundingClientRect().width;
	const currentFlexes = panes.flexes;
	const availableWidth = containerWidth - (panes.list.length - 1) * 4;
	const totalFlex = currentFlexes.reduce((a, b) => a + b, 0);
	const leftStartPx = (currentFlexes[dividerIndex] / totalFlex) * availableWidth;
	const rightStartPx = (currentFlexes[dividerIndex + 1] / totalFlex) * availableWidth;
	const minPx = 120;

	function onMove(ev: MouseEvent) {
		const delta = ev.clientX - startX;
		const leftPx = Math.max(
			minPx,
			Math.min(leftStartPx + delta, leftStartPx + rightStartPx - minPx)
		);
		const rightPx = leftStartPx + rightStartPx - leftPx;
		panes.flexes = currentFlexes.map((f, i) => {
			if (i === dividerIndex) return (leftPx / availableWidth) * totalFlex;
			if (i === dividerIndex + 1) return (rightPx / availableWidth) * totalFlex;
			return f;
		});
	}
	function onUp() {
		setResizing(false);
		window.removeEventListener('mousemove', onMove);
		window.removeEventListener('mouseup', onUp);
	}
	window.addEventListener('mousemove', onMove);
	window.addEventListener('mouseup', onUp);
}
