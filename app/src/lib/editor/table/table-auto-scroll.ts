import type { DraggingDOMs } from './table-hit-test';

const EDGE_THRESHOLD = 80;
const SCROLL_SPEED = 12;
const FRAME_MS = 16;

export interface TableAutoScroll {
	check(clientX: number, clientY: number, doms?: DraggingDOMs): void;
	stop(): void;
}

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

export function createTableAutoScroll(start: HTMLElement): TableAutoScroll {
	const scrollParent = findScrollParent(start);
	let interval: number | undefined;

	function stop() {
		if (interval) {
			clearInterval(interval);
			interval = undefined;
		}
	}

	function check(clientX: number, clientY: number, doms?: DraggingDOMs) {
		stop();
		if (scrollParent) {
			const rect = scrollParent.getBoundingClientRect();
			if (clientY < rect.top + EDGE_THRESHOLD) {
				interval = window.setInterval(() => {
					scrollParent.scrollTop = Math.max(0, scrollParent.scrollTop - SCROLL_SPEED);
				}, FRAME_MS);
				return;
			}
			if (clientY > rect.bottom - EDGE_THRESHOLD) {
				interval = window.setInterval(() => {
					scrollParent.scrollTop = Math.min(
						scrollParent.scrollHeight,
						scrollParent.scrollTop + SCROLL_SPEED
					);
				}, FRAME_MS);
				return;
			}
		}
		if (doms) {
			const wrapper = doms.table.closest<HTMLElement>('.tableWrapper');
			if (wrapper) {
				const rect = wrapper.getBoundingClientRect();
				if (clientX < rect.left + EDGE_THRESHOLD) {
					interval = window.setInterval(() => {
						wrapper.scrollLeft -= SCROLL_SPEED;
					}, FRAME_MS);
					return;
				}
				if (clientX > rect.right - EDGE_THRESHOLD) {
					interval = window.setInterval(() => {
						wrapper.scrollLeft += SCROLL_SPEED;
					}, FRAME_MS);
				}
			}
		}
	}

	return { check, stop };
}
