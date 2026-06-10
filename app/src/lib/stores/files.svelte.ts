import { SvelteSet } from "svelte/reactivity";
import { buildVisibleTree, buildSubtree, type TreeEntry } from "$lib/fs/bridge";

export type SortOrder = "name" | "date";
export type TreeRevealTarget =
  | { kind: "entry"; path: string }
  | { kind: "pending-new-folder"; parentPath: string };
export type { TreeEntry };

export interface SelectedEntry {
  path: string;
  isDir: boolean;
}

interface FilesState {
  /** Flat, sorted, depth-annotated visible tree rows. */
  flatTree: TreeEntry[];
  vaultRoot: string | null;
  activeFile: string | null;
  selectedFolder: string | null;
  lastSelectedPath: string | null;
  loading: boolean;
  expandedFolders: SvelteSet<string>;
  pendingNewFolder: string | null;
  renamingPath: string | null;
  treeRevealTarget: TreeRevealTarget | null;
  treeRevealVersion: number;
  sortOrder: SortOrder;
}

let state = $state<FilesState>({
  flatTree: [],
  vaultRoot: null,
  activeFile: null,
  selectedFolder: null,
  lastSelectedPath: null,
  loading: false,
  expandedFolders: new SvelteSet(),
  pendingNewFolder: null,
  renamingPath: null,
  treeRevealTarget: null,
  treeRevealVersion: 0,
  sortOrder: "name",
});

/**
 * Selection lives in a separate $state.raw Map so it is NOT deeply proxied by
 * Svelte. Mutations reassign the container (`selectedEntries = ...`) to notify
 * dependents. Because the container identity changes (not every nested entry),
 * rows that read `isSelected(path)` only re-evaluate the cheap `.has` lookup.
 */
let selectedEntries = $state.raw<Map<string, SelectedEntry>>(new Map());

async function _rebuild(): Promise<void> {
  if (!state.vaultRoot) return;
  const expanded = [...state.expandedFolders];
  const flatTree = await buildVisibleTree(
    state.vaultRoot,
    expanded,
    state.sortOrder,
  );
  state.flatTree = flatTree;
  _pathIndexDirty = true;
}

let _pathIndex = new Map<string, number>();
// _pathIndex is only consumed by selectRange. Rebuild it lazily on demand so
// user-paced expand/collapse/rebuild operations don't pay an O(n) Map rebuild
// each time when no range selection ever follows.
let _pathIndexDirty = true;

function pathIndex(): Map<string, number> {
  if (_pathIndexDirty) {
    _pathIndex = new Map(state.flatTree.map((r, i) => [r.path, i]));
    _pathIndexDirty = false;
  }
  return _pathIndex;
}

function _requestTreeReveal(target: TreeRevealTarget) {
  state.treeRevealTarget = target;
  state.treeRevealVersion += 1;
}

export const files = {
  get flatTree() {
    return state.flatTree;
  },
  get activeFile() {
    return state.activeFile;
  },
  get selectedFolder() {
    return state.selectedFolder;
  },
  get loading() {
    return state.loading;
  },
  get expandedFolders() {
    return state.expandedFolders;
  },

  setActiveFile(path: string | null) {
    state.activeFile = path;
  },

  async revealFile(filePath: string, vaultPath: string) {
    const rel = filePath.slice(vaultPath.length + 1);
    const parts = rel.split("/");
    const before = state.expandedFolders.size;
    let current = vaultPath;
    for (let i = 0; i < parts.length - 1; i++) {
      current += "/" + parts[i];
      state.expandedFolders.add(current);
    }
    // Only re-walk the tree when an ancestor was actually newly expanded. On the
    // common path (tab switch / opening a file whose ancestors are already
    // expanded) nothing was added, so skip the build_visible_tree IPC walk and
    // just fire a scroll request.
    if (state.expandedFolders.size !== before) {
      await _rebuild();
    }
    _requestTreeReveal({ kind: "entry", path: filePath });
  },

  requestTreeReveal(path: string) {
    _requestTreeReveal({ kind: "entry", path });
  },

  requestPendingNewFolderReveal(parentPath: string) {
    _requestTreeReveal({ kind: "pending-new-folder", parentPath });
  },

  get treeRevealTarget() {
    return state.treeRevealTarget;
  },

  get treeRevealVersion() {
    return state.treeRevealVersion;
  },

  setSelectedFolder(path: string | null) {
    state.selectedFolder = path;
  },

  // ─── Multi-selection ───

  get selectedEntries() {
    return selectedEntries;
  },

  get selectedEntry(): SelectedEntry | null {
    if (selectedEntries.size === 1) {
      return selectedEntries.values().next().value!;
    }
    return selectedEntries.size > 0
      ? { path: state.lastSelectedPath!, isDir: selectedEntries.get(state.lastSelectedPath!)?.isDir ?? false }
      : null;
  },

  get lastSelectedPath() {
    return state.lastSelectedPath;
  },

  selectSingle(path: string, isDir: boolean) {
    selectedEntries = new Map([[path, { path, isDir }]]);
    state.lastSelectedPath = path;
  },

  selectToggle(path: string, isDir: boolean) {
    const next = new Map(selectedEntries);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.set(path, { path, isDir });
    }
    selectedEntries = next;
    state.lastSelectedPath = path;
  },

  selectRange(path: string) {
    const anchor = state.lastSelectedPath;
    if (!anchor) {
      const row = state.flatTree.find((r) => r.path === path);
      if (row) {
        selectedEntries = new Map([[path, { path, isDir: row.is_dir }]]);
        state.lastSelectedPath = path;
      }
      return;
    }
    const flat = state.flatTree;
    const idx = pathIndex();
    const anchorIdx = idx.get(anchor);
    const targetIdx = idx.get(path);
    if (anchorIdx === undefined || targetIdx === undefined) return;
    const lo = Math.min(anchorIdx, targetIdx);
    const hi = Math.max(anchorIdx, targetIdx);
    const next = new Map<string, SelectedEntry>();
    for (let i = lo; i <= hi; i++) {
      next.set(flat[i].path, { path: flat[i].path, isDir: flat[i].is_dir });
    }
    selectedEntries = next;
  },

  isSelected(path: string) {
    return selectedEntries.has(path);
  },

  getSelectedPaths(): string[] {
    return [...selectedEntries.keys()];
  },

  getSelectedAsList(): SelectedEntry[] {
    return [...selectedEntries.values()];
  },

  clearSelection() {
    selectedEntries = new Map();
    state.lastSelectedPath = null;
  },

  selectAll() {
    const next = new Map<string, SelectedEntry>();
    for (const row of state.flatTree) {
      next.set(row.path, { path: row.path, isDir: row.is_dir });
    }
    selectedEntries = next;
  },

  setSelectedEntry(path: string, isDir: boolean) {
    this.selectSingle(path, isDir);
  },

  async expandFolder(path: string) {
    state.expandedFolders.add(path);
    const idx = state.flatTree.findIndex((r) => r.path === path);
    if (idx !== -1 && state.vaultRoot) {
      const parentDepth = state.flatTree[idx].depth;
      const children = await buildSubtree(
        path,
        parentDepth + 1,
        [...state.expandedFolders],
        state.sortOrder,
      );
      const next = [...state.flatTree];
      next.splice(idx + 1, 0, ...children);
      state.flatTree = next;
      _pathIndexDirty = true;
    } else {
      await _rebuild();
    }
  },

  async collapseAll() {
    state.expandedFolders.clear();
    try {
      await _rebuild();
    } catch (err) {
      console.warn("Failed to rebuild tree after collapseAll:", err);
    }
  },

  async collapseFolder(path: string) {
    state.expandedFolders.delete(path);
    const idx = state.flatTree.findIndex((r) => r.path === path);
    if (idx !== -1) {
      const parentDepth = state.flatTree[idx].depth;
      let end = idx + 1;
      while (end < state.flatTree.length && state.flatTree[end].depth > parentDepth) {
        end++;
      }
      if (end > idx + 1) {
        const next = [...state.flatTree];
        next.splice(idx + 1, end - idx - 1);
        state.flatTree = next;
        _pathIndexDirty = true;
      }
    } else {
      await _rebuild();
    }
  },

  async toggleFolder(path: string) {
    if (state.expandedFolders.has(path)) {
      await this.collapseFolder(path);
    } else {
      await this.expandFolder(path);
    }
  },

  async refresh(vaultPath?: string) {
    if (vaultPath) state.vaultRoot = vaultPath;
    if (!state.vaultRoot) return;
    state.loading = true;
    try {
      await _rebuild();
    } finally {
      state.loading = false;
    }
  },

  clear() {
    state.flatTree = [];
    state.vaultRoot = null;
    state.activeFile = null;
    state.selectedFolder = null;
    selectedEntries = new Map();
    state.lastSelectedPath = null;
    state.expandedFolders.clear();
    state.pendingNewFolder = null;
    state.renamingPath = null;
    state.treeRevealTarget = null;
    state.treeRevealVersion = 0;
    _pathIndex = new Map();
    _pathIndexDirty = false;
  },

  get pendingNewFolder() {
    return state.pendingNewFolder;
  },

  get sortOrder() {
    return state.sortOrder;
  },

  async setSortOrder(order: SortOrder) {
    state.sortOrder = order;
    await _rebuild();
  },

  async toggleSortOrder() {
    state.sortOrder = state.sortOrder === "name" ? "date" : "name";
    await _rebuild();
  },

  startNewFolder(parentPath: string) {
    state.pendingNewFolder = parentPath;
  },

  cancelNewFolder() {
    state.pendingNewFolder = null;
  },

  get renamingPath() {
    return state.renamingPath;
  },

  startRename(path: string) {
    state.renamingPath = path;
  },

  cancelRename() {
    state.renamingPath = null;
  },
};
