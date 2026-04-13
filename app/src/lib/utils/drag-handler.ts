import { drag } from "$lib/stores/drag.svelte";
import type { DragItem } from "$lib/stores/drag.svelte";

/**
 * Start a pointer-based drag operation after a 4px movement threshold.
 * Returns a cleanup function (not usually needed — listeners auto-remove on mouseup).
 */
export function startPointerDrag(
  e: MouseEvent,
  item: DragItem,
  onDragStart?: () => void,
): void {
  if (e.button !== 0) return;
  const startX = e.clientX;
  const startY = e.clientY;
  let didDrag = false;

  function onMove(ev: MouseEvent) {
    if (
      !didDrag &&
      (Math.abs(ev.clientX - startX) > 4 || Math.abs(ev.clientY - startY) > 4)
    ) {
      didDrag = true;
      drag.start(item, ev.clientX, ev.clientY);
      onDragStart?.();
      window.removeEventListener("mousemove", onMove);
    }
  }
  function onUp() {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  }
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}
