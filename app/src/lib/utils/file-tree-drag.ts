import { drag } from "$lib/stores/drag.svelte";
import { files } from "$lib/stores/files.svelte";
import { startPointerDrag } from "$lib/utils/drag-handler";
import { startDrag as startNativeDrag } from "@crabnebula/tauri-plugin-drag";
import type { CallbackPayload } from "@crabnebula/tauri-plugin-drag";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { TreeEntry } from "$lib/fs/bridge";

const NATIVE_DRAG_WINDOW_MARGIN = 4;
const NATIVE_DRAG_DROP_SETTLE_MS = 50;
const NATIVE_DRAG_END_SETTLE_MS = 100;

export function isDescendantOrSelf(source: string, target: string): boolean {
  return target === source || target.startsWith(source + "/");
}

export function getDropEntries(): { path: string; isDir: boolean }[] {
  const item = drag.item;
  if (!item || item.kind !== "file") return [];
  if (files.selectedEntries.size > 1 && files.isSelected(item.path)) {
    return files.getSelectedAsList();
  }
  return [{ path: item.path, isDir: item.isDir }];
}

export function handleFolderDrop(
  e: MouseEvent,
  folderPath: string,
  onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>,
  setDropTarget: (path: string | null) => void,
) {
  if (!drag.active || !drag.item || drag.item.kind !== "file") return;
  e.stopPropagation();
  const entries = getDropEntries();
  const valid = entries.filter((entry) => {
    if (isDescendantOrSelf(entry.path, folderPath)) return false;
    const parent = entry.path.slice(0, entry.path.lastIndexOf("/"));
    return parent !== folderPath;
  });
  drag.end();
  setDropTarget(null);
  for (const entry of valid) {
    onmoveentry(entry.path, folderPath, entry.isDir);
  }
}

export function handleRootDrop(
  e: MouseEvent,
  vaultPath: string,
  onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>,
  setDropTarget: (path: string | null) => void,
) {
  if (!drag.active || !drag.item || drag.item.kind !== "file") return;
  const entries = getDropEntries();
  const valid = entries.filter((entry) => {
    const parent = entry.path.slice(0, entry.path.lastIndexOf("/"));
    return parent !== vaultPath;
  });
  drag.end();
  setDropTarget(null);
  for (const entry of valid) {
    onmoveentry(entry.path, vaultPath, entry.isDir);
  }
}

async function isCursorOutsideCurrentWindow(
  cursorPos: CallbackPayload["cursorPos"],
): Promise<boolean> {
  const x = Number(cursorPos.x);
  const y = Number(cursorPos.y);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return false;

  try {
    const currentWindow = getCurrentWindow();
    const [position, size] = await Promise.all([
      currentWindow.outerPosition(),
      currentWindow.outerSize(),
    ]);

    return (
      x < position.x - NATIVE_DRAG_WINDOW_MARGIN ||
      x > position.x + size.width + NATIVE_DRAG_WINDOW_MARGIN ||
      y < position.y - NATIVE_DRAG_WINDOW_MARGIN ||
      y > position.y + size.height + NATIVE_DRAG_WINDOW_MARGIN
    );
  } catch {
    return false;
  }
}

async function shouldDeleteAfterNativeDrop(
  cursorPos: CallbackPayload["cursorPos"],
): Promise<boolean> {
  await new Promise<void>((resolve) =>
    window.setTimeout(resolve, NATIVE_DRAG_DROP_SETTLE_MS),
  );
  if (drag.droppedBackInApp) return false;
  return isCursorOutsideCurrentWindow(cursorPos);
}

/** Start native OS drag when cursor exits the window. */
export function tryNativeDrag(
  clientX: number,
  clientY: number,
  dragIconPath: string,
  ondeleteentry: (path: string, isDir: boolean) => Promise<void>,
  nativeDragState: { started: boolean },
) {
  if (!drag.active || drag.nativeDragActive || nativeDragState.started) return;
  const item = drag.item;
  if (!item || item.kind !== "file") return;

  const margin = 2;
  const outside =
    clientX <= margin ||
    clientY <= margin ||
    clientX >= window.innerWidth - margin ||
    clientY >= window.innerHeight - margin;
  if (!outside) return;

  nativeDragState.started = true;
  drag.end();
  drag.startNativeDrag();

  const paths =
    files.selectedEntries.size > 1 && files.isSelected(item.path)
      ? files.getSelectedPaths()
      : [item.path];

  const dragEntries =
    files.selectedEntries.size > 1 && files.isSelected(item.path)
      ? files.getSelectedAsList()
      : [{ path: item.path, isDir: item.isDir }];

  let finished = false;
  function finishNativeDrag() {
    if (finished) return;
    finished = true;
    nativeDragState.started = false;
    window.setTimeout(() => {
      drag.endNativeDrag();
    }, NATIVE_DRAG_END_SETTLE_MS);
  }

  startNativeDrag(
    { item: paths, icon: dragIconPath },
    ({ result, cursorPos }) => {
      if (result !== "Dropped") {
        finishNativeDrag();
        return;
      }

      void shouldDeleteAfterNativeDrop(cursorPos).then((shouldDelete) => {
        if (!shouldDelete) return;
        for (const entry of dragEntries) {
          ondeleteentry(entry.path, entry.isDir).catch(() => {});
        }
      });
      finishNativeDrag();
    },
  )
    .catch(() => {
      finishNativeDrag();
    });
}

export function startDragEntry(
  e: MouseEvent,
  entry: TreeEntry,
  onDragStart: () => void,
) {
  const label = entry.name.replace(/\.(md|canvas)$/, "");
  startPointerDrag(
    e,
    { kind: "file", path: entry.path, label, isDir: entry.is_dir },
    onDragStart,
  );
}
