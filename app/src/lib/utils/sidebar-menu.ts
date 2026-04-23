import type { ContextMenuItem } from "$lib/components/ContextMenu.svelte";
import type { FsEntry, TreeEntry } from "$lib/fs/bridge";
import { favourites } from "$lib/stores/favourites.svelte";
import { clipboard } from "$lib/stores/clipboard.svelte";
import { files } from "$lib/stores/files.svelte";
import * as m from "$lib/paraglide/messages.js";

export type MenuTarget =
  | { kind: "root"; path: string }
  | { kind: "entry"; entry: TreeEntry | FsEntry };

interface MenuHandlers {
  onNewFile: (base: string) => void;
  onNewCanvas: (base: string) => void;
  onNewFolder: (base: string) => void;
  onPaste: (dir: string) => void;
  onOpenInFinder: (path: string) => void;
  onCopy: (entry: FsEntry) => void;
  onCut: (entry: FsEntry) => void;
  onRename: (path: string) => void;
  onDuplicate: (entry: FsEntry) => void;
  onDelete: (entry: FsEntry) => void;
}

export function buildMenuItems(
  target: MenuTarget,
  handlers: MenuHandlers,
): ContextMenuItem[] {
  if (target.kind === "root") {
    const targetPath = target.path;
    return [
      { label: m.sidebar_new_file(), onclick: () => handlers.onNewFile(targetPath) },
      { label: m.sidebar_new_canvas(), onclick: () => handlers.onNewCanvas(targetPath) },
      { label: m.sidebar_new_folder(), onclick: () => handlers.onNewFolder(targetPath) },
      ...(clipboard.hasItems
        ? [{ label: m.sidebar_paste(), onclick: () => handlers.onPaste(targetPath) }]
        : []),
      { label: m.sidebar_open_in_finder(), onclick: () => handlers.onOpenInFinder(targetPath) },
    ];
  }

  const entry = target.entry;
  const items: ContextMenuItem[] = [];
  if (entry.is_dir) {
    items.push(
      { label: m.sidebar_new_file(), onclick: () => handlers.onNewFile(entry.path) },
      { label: m.sidebar_new_canvas(), onclick: () => handlers.onNewCanvas(entry.path) },
      { label: m.sidebar_new_folder(), onclick: () => handlers.onNewFolder(entry.path) },
    );
  } else {
    items.push({
      label: favourites.isFavourite(entry.path)
        ? m.sidebar_remove_favourite()
        : m.sidebar_add_favourite(),
      onclick: () => favourites.toggle(entry.path),
    });
  }
  items.push(
    { label: m.sidebar_copy(), onclick: () => handlers.onCopy(entry) },
    { label: m.sidebar_cut(), onclick: () => handlers.onCut(entry) },
  );
  if (clipboard.hasItems) {
    const pasteDir = entry.is_dir
      ? entry.path
      : entry.path.slice(0, entry.path.lastIndexOf("/"));
    items.push({ label: m.sidebar_paste(), onclick: () => handlers.onPaste(pasteDir) });
  }
  items.push(
    { label: m.sidebar_rename(), onclick: () => files.startRename(entry.path) },
    { label: m.sidebar_duplicate(), onclick: () => handlers.onDuplicate(entry) },
    { label: m.sidebar_delete(), onclick: () => handlers.onDelete(entry), destructive: true },
    { label: m.sidebar_open_in_finder(), onclick: () => handlers.onOpenInFinder(entry.path) },
  );
  return items;
}
