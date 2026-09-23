import { drag } from '$lib/stores/drag.svelte';
import type { DragItem } from '$lib/stores/drag.svelte';

const DRAG_THRESHOLD_PX = 4;

export function startPointerDrag(
	e: MouseEvent,
	item: DragItem,
	onDragStart?: () => void,
	onClick?: () => void
): void {
	if (e.button !== 0) return;
	const startX = e.clientX;
	const startY = e.clientY;
	let didDrag = false;

	const ac = new AbortController();
	const { signal } = ac;

	function onMove(ev: MouseEvent) {
		if (
			!didDrag &&
			(Math.abs(ev.clientX - startX) > DRAG_THRESHOLD_PX ||
				Math.abs(ev.clientY - startY) > DRAG_THRESHOLD_PX)
		) {
			didDrag = true;
			drag.start(item, ev.clientX, ev.clientY);
			onDragStart?.();
		}
	}
	function onUp() {
		ac.abort();
		if (!didDrag) onClick?.();
	}
	window.addEventListener('mousemove', onMove, { signal });
	window.addEventListener('mouseup', onUp, { signal });
}
