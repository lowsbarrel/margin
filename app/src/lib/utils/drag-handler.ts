import { drag } from "$lib/stores/drag.svelte";
import type { DragItem } from "$lib/stores/drag.svelte";

/**
 * Start a pointer-based drag after a 4px movement threshold.
 *
 * @param onDragStart  Invoked once the drag threshold is crossed.
 * @param onClick      Invoked on mouseup when no drag occurred (a plain click).
 */
export function startPointerDrag(
  e: MouseEvent,
  item: DragItem,
  onDragStart?: () => void,
  onClick?: () => void,
): void {
  if (e.button !== 0) return;
  const startX = e.clientX;
  const startY = e.clientY;
  let didDrag = false;

  // A single AbortController removes both listeners atomically on drop, avoiding
  // the asymmetric teardown where mousemove was removed on threshold-cross but
  // mouseup only on fire.
  const ac = new AbortController();
  const { signal } = ac;

  function onMove(ev: MouseEvent) {
    if (
      !didDrag &&
      (Math.abs(ev.clientX - startX) > 4 || Math.abs(ev.clientY - startY) > 4)
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
  window.addEventListener("mousemove", onMove, { signal });
  window.addEventListener("mouseup", onUp, { signal });
}
