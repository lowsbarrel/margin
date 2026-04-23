import { drag } from "$lib/stores/drag.svelte";
import { files } from "$lib/stores/files.svelte";
import { startPointerDrag } from "$lib/utils/drag-handler";
import { startDrag as startNativeDrag } from "@crabnebula/tauri-plugin-drag";
import type { TreeEntry } from "$lib/fs/bridge";

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

/** Start native OS drag when cursor exits the window. */
export function tryNativeDrag(
  clientX: number,
  clientY: number,
  dragIconPath: string,
  ondeleteentry: (path: string, isDir: boolean) => Promise<void>,
  nativeDragState: { started: boolean },
) {
  if (!drag.active || nativeDragState.started) return;
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

  const paths =
    files.selectedEntries.size > 1 && files.isSelected(item.path)
      ? files.getSelectedPaths()
      : [item.path];

  const dragEntries =
    files.selectedEntries.size > 1 && files.isSelected(item.path)
      ? files.getSelectedAsList()
      : [{ path: item.path, isDir: item.isDir }];

  startNativeDrag(
    { item: paths, icon: dragIconPath },
    ({ result }) => {
      if (result === "Dropped") {
        for (const entry of dragEntries) {
          ondeleteentry(entry.path, entry.isDir).catch(() => {});
        }
      }
    },
  )
    .catch(() => {})
    .finally(() => {
      nativeDragState.started = false;
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
