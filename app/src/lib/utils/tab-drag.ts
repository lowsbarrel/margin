import { panes, fileTitle } from "$lib/stores/panes.svelte";
import { drag } from "$lib/stores/drag.svelte";

export function handleTabMouseDown(
  e: MouseEvent,
  paneIndex: number,
  tabIndex: number,
) {
  if (e.button !== 0) return;
  if (panes.list.length === 1 && panes.list[0].tabs.length === 1) return;
  e.preventDefault();
  const startX = e.clientX,
    startY = e.clientY;
  let didDrag = false;
  const label = fileTitle(panes.list[paneIndex].tabs[tabIndex].path);

  function onMove(ev: MouseEvent) {
    if (
      !didDrag &&
      (Math.abs(ev.clientX - startX) > 4 || Math.abs(ev.clientY - startY) > 4)
    ) {
      didDrag = true;
      drag.start(
        { kind: "tab", paneIndex, tabIndex, label },
        ev.clientX,
        ev.clientY,
      );
      window.removeEventListener("mousemove", onMove);
    }
  }
  function onUp() {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    if (!didDrag) panes.switchTab(paneIndex, tabIndex);
  }
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

export async function executeDrop(target: {
  paneIndex: number;
  zone: "left" | "center" | "right";
}) {
  const item = drag.item;
  if (!item) return;
  if (item.kind === "file") {
    if (item.isDir) return;
    if (target.zone === "center") {
      if (target.paneIndex !== panes.activePaneIndex)
        await panes.focusPane(target.paneIndex);
      await panes.openFile(item.path);
    } else {
      await panes.openFileInNewPane(item.path, target.paneIndex, target.zone);
    }
  } else if (item.kind === "tab") {
    const { paneIndex: srcPane, tabIndex: srcTab } = item;
    if (target.zone === "center")
      await panes.moveTabToPane(srcPane, srcTab, target.paneIndex);
    else
      await panes.moveTabToNewPane(srcPane, srcTab, target.paneIndex, target.zone);
  }
}

export function startDividerDrag(
  e: MouseEvent,
  dividerIndex: number,
  panesContainerEl: HTMLElement,
  setResizing: (v: boolean) => void,
) {
  e.preventDefault();
  setResizing(true);
  const startX = e.clientX;
  const containerWidth = panesContainerEl.getBoundingClientRect().width;
  const currentFlexes = panes.flexes;
  const availableWidth = containerWidth - (panes.list.length - 1) * 4;
  const totalFlex = currentFlexes.reduce((a, b) => a + b, 0);
  const leftStartPx = (currentFlexes[dividerIndex] / totalFlex) * availableWidth;
  const rightStartPx =
    (currentFlexes[dividerIndex + 1] / totalFlex) * availableWidth;
  const minPx = 120;

  function onMove(ev: MouseEvent) {
    const delta = ev.clientX - startX;
    const leftPx = Math.max(
      minPx,
      Math.min(leftStartPx + delta, leftStartPx + rightStartPx - minPx),
    );
    const rightPx = leftStartPx + rightStartPx - leftPx;
    panes.flexes = currentFlexes.map((f, i) => {
      if (i === dividerIndex) return (leftPx / availableWidth) * totalFlex;
      if (i === dividerIndex + 1)
        return (rightPx / availableWidth) * totalFlex;
      return f;
    });
  }
  function onUp() {
    setResizing(false);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  }
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}
