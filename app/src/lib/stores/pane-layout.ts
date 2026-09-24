import type { Pane, Tab } from '$lib/stores/panes.svelte';
import { nextPaneId } from '$lib/stores/pane-tabs';

export type DropSide = 'left' | 'right';
export type FlexSink = 'neighbour' | number;

export interface LayoutMove {
	panes: Pane[];
	flexes: number[];
	activePaneIndex: number;
	tab: Tab;
}

export function clampInsertIndex(tabs: Tab[], pinned: boolean, index: number): number {
	const pinnedCount = tabs.filter((t) => t.pinned).length;
	return pinned
		? Math.max(0, Math.min(index, pinnedCount))
		: Math.max(pinnedCount, Math.min(index, tabs.length));
}

export function reorderTabGap(tabs: Tab[], from: number, gap: number): number {
	const tab = tabs[from];
	if (!tab) return gap;
	const rest = tabs.filter((_, i) => i !== from);
	const to = clampInsertIndex(rest, tab.pinned, gap > from ? gap - 1 : gap);
	return to <= from ? to : to + 1;
}

export function reorderTab(panes: Pane[], paneIndex: number, from: number, gap: number): void {
	const pane = panes[paneIndex];
	const tab = pane?.tabs[from];
	if (!tab) return;
	const droppable = reorderTabGap(pane.tabs, from, gap);
	if (droppable === from || droppable === from + 1) return;
	const activeId = pane.tabs[pane.activeTabIndex]?.id ?? null;
	const next = pane.tabs.filter((_, i) => i !== from);
	next.splice(droppable > from ? droppable - 1 : droppable, 0, tab);
	pane.tabs = next;
	pane.activeTabIndex = activeId === null ? -1 : next.findIndex((t) => t.id === activeId);
}

export function removeFlexAt(flexes: number[], index: number): number[] {
	const removed = flexes[index];
	const next = flexes.filter((_, i) => i !== index);
	const ni = Math.min(index > 0 ? index - 1 : 0, next.length - 1);
	if (next.length > 0) next[ni] += removed;
	return next;
}

export function splitAtPane(
	flexes: number[],
	refPaneIndex: number,
	side: DropSide
): { flexes: number[]; insertAt: number } {
	const insertAt = side === 'left' ? refPaneIndex : refPaneIndex + 1;
	const half = flexes[refPaneIndex] / 2;
	const next = [...flexes];
	next[refPaneIndex] = half;
	next.splice(insertAt, 0, half);
	return { flexes: next, insertAt };
}

export function detachTab(
	panes: Pane[],
	srcPaneIndex: number,
	srcTabIndex: number
): { tab: Tab; emptied: boolean } | null {
	const pane = panes[srcPaneIndex];
	const tab = pane?.tabs[srcTabIndex];
	if (!tab) return null;
	const active = pane.activeTabIndex;
	pane.tabs = pane.tabs.filter((_, i) => i !== srcTabIndex);
	if (pane.tabs.length === 0) pane.activeTabIndex = -1;
	else if (srcTabIndex === active)
		pane.activeTabIndex = Math.min(srcTabIndex, pane.tabs.length - 1);
	else if (srcTabIndex < active) pane.activeTabIndex--;
	return { tab, emptied: pane.tabs.length === 0 };
}

export function removePaneAt(
	panes: Pane[],
	flexes: number[],
	index: number,
	flexSink: FlexSink
): void {
	const removed = flexes[index];
	flexes.splice(index, 1);
	panes.splice(index, 1);
	if (flexes.length === 0) return;
	const target = flexSink === 'neighbour' ? (index > 0 ? index - 1 : 0) : flexSink;
	flexes[Math.min(target, flexes.length - 1)] += removed;
}

export function moveTabIntoPane(
	panes: Pane[],
	flexes: number[],
	srcPaneIndex: number,
	srcTabIndex: number,
	destPaneIndex: number,
	insertIndex?: number
): LayoutMove | null {
	const workPanes: Pane[] = panes.map((p) => ({ ...p, tabs: [...p.tabs] }));
	const workFlexes: number[] = [...flexes];
	const detached = detachTab(workPanes, srcPaneIndex, srcTabIndex);
	if (!detached) return null;

	let actualDest = destPaneIndex;
	if (detached.emptied) {
		if (srcPaneIndex < destPaneIndex) actualDest--;
		removePaneAt(workPanes, workFlexes, srcPaneIndex, 'neighbour');
	}

	const destPane = workPanes[actualDest];
	const at = clampInsertIndex(
		destPane.tabs,
		detached.tab.pinned,
		insertIndex ?? destPane.tabs.length
	);
	destPane.tabs = [...destPane.tabs.slice(0, at), detached.tab, ...destPane.tabs.slice(at)];
	destPane.activeTabIndex = at;

	return {
		panes: workPanes,
		flexes: workFlexes,
		activePaneIndex: Math.min(actualDest, workPanes.length - 1),
		tab: detached.tab
	};
}

export function splitTabToNewPane(
	panes: Pane[],
	flexes: number[],
	srcPaneIndex: number,
	srcTabIndex: number,
	refPaneIndex: number,
	side: DropSide
): LayoutMove | null {
	const workPanes: Pane[] = panes.map((p) => ({ ...p, tabs: [...p.tabs] }));
	const workFlexes: number[] = [...flexes];
	const detached = detachTab(workPanes, srcPaneIndex, srcTabIndex);
	if (!detached) return null;

	let adjustedRef = refPaneIndex;
	if (detached.emptied) {
		if (srcPaneIndex < refPaneIndex) adjustedRef--;
		removePaneAt(workPanes, workFlexes, srcPaneIndex, adjustedRef);
	}

	const { flexes: nextFlexes, insertAt } = splitAtPane(workFlexes, adjustedRef, side);
	workPanes.splice(insertAt, 0, {
		id: nextPaneId(),
		tabs: [detached.tab],
		activeTabIndex: 0,
		externalContentVersion: 0
	});

	return {
		panes: workPanes,
		flexes: nextFlexes,
		activePaneIndex: insertAt,
		tab: detached.tab
	};
}
