import { buildVisibleTree, buildSubtree, type TreeEntry } from "$lib/fs/bridge";

export type SortOrder = "name" | "date";
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
  selectedEntries: Map<string, SelectedEntry>;
  lastSelectedPath: string | null;
  loading: boolean;
  expandedFolders: Set<string>;
  pendingNewFolder: string | null;
  renamingPath: string | null;
  sortOrder: SortOrder;
}

let state = $state<FilesState>({
  flatTree: [],
  vaultRoot: null,
  activeFile: null,
  selectedFolder: null,
  selectedEntries: new Map(),
  lastSelectedPath: null,
  loading: false,
  expandedFolders: new Set(),
  pendingNewFolder: null,
  renamingPath: null,
  sortOrder: "name",
});

async function _rebuild(): Promise<void> {
  if (!state.vaultRoot) return;
  const expanded = [...state.expandedFolders];
  const flatTree = await buildVisibleTree(
    state.vaultRoot,
    expanded,
    state.sortOrder,
  );
  state.flatTree = flatTree;
  _pathIndex = new Map(flatTree.map((r, i) => [r.path, i]));
}

let _pathIndex = new Map<string, number>();

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
    let current = vaultPath;
    for (let i = 0; i < parts.length - 1; i++) {
      current += "/" + parts[i];
      state.expandedFolders.add(current);
    }
    state.expandedFolders = new Set(state.expandedFolders);
    await _rebuild();
  },

  setSelectedFolder(path: string | null) {
    state.selectedFolder = path;
  },

  // ─── Multi-selection ───

  get selectedEntries() {
    return state.selectedEntries;
  },

  get selectedEntry(): SelectedEntry | null {
    if (state.selectedEntries.size === 1) {
      return state.selectedEntries.values().next().value!;
    }
    return state.selectedEntries.size > 0
      ? { path: state.lastSelectedPath!, isDir: state.selectedEntries.get(state.lastSelectedPath!)?.isDir ?? false }
      : null;
  },

  get lastSelectedPath() {
    return state.lastSelectedPath;
  },

  selectSingle(path: string, isDir: boolean) {
    state.selectedEntries = new Map([[path, { path, isDir }]]);
    state.lastSelectedPath = path;
  },

  selectToggle(path: string, isDir: boolean) {
    const next = new Map(state.selectedEntries);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.set(path, { path, isDir });
    }
    state.selectedEntries = next;
    state.lastSelectedPath = path;
  },

  selectRange(path: string) {
    const anchor = state.lastSelectedPath;
    if (!anchor) {
      const row = state.flatTree.find((r) => r.path === path);
      if (row) {
        state.selectedEntries = new Map([[path, { path, isDir: row.is_dir }]]);
        state.lastSelectedPath = path;
      }
      return;
    }
    const flat = state.flatTree;
    const anchorIdx = _pathIndex.get(anchor);
    const targetIdx = _pathIndex.get(path);
    if (anchorIdx === undefined || targetIdx === undefined) return;
    const lo = Math.min(anchorIdx, targetIdx);
    const hi = Math.max(anchorIdx, targetIdx);
    const next = new Map<string, SelectedEntry>();
    for (let i = lo; i <= hi; i++) {
      next.set(flat[i].path, { path: flat[i].path, isDir: flat[i].is_dir });
    }
    state.selectedEntries = next;
  },

  isSelected(path: string) {
    return state.selectedEntries.has(path);
  },

  getSelectedPaths(): string[] {
    return [...state.selectedEntries.keys()];
  },

  getSelectedAsList(): SelectedEntry[] {
    return [...state.selectedEntries.values()];
  },

  clearSelection() {
    state.selectedEntries = new Map();
    state.lastSelectedPath = null;
  },

  selectAll() {
    const next = new Map<string, SelectedEntry>();
    for (const row of state.flatTree) {
      next.set(row.path, { path: row.path, isDir: row.is_dir });
    }
    state.selectedEntries = next;
  },

  setSelectedEntry(path: string, isDir: boolean) {
    this.selectSingle(path, isDir);
  },

  async expandFolder(path: string) {
    state.expandedFolders.add(path);
    state.expandedFolders = new Set(state.expandedFolders);
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
      _pathIndex = new Map(state.flatTree.map((r, i) => [r.path, i]));
    } else {
      await _rebuild();
    }
  },

  collapseAll() {
    state.expandedFolders = new Set();
    _rebuild();
  },

  async collapseFolder(path: string) {
    state.expandedFolders.delete(path);
    state.expandedFolders = new Set(state.expandedFolders);
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
        _pathIndex = new Map(state.flatTree.map((r, i) => [r.path, i]));
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
    state.selectedEntries = new Map();
    state.lastSelectedPath = null;
    state.expandedFolders = new Set();
    state.pendingNewFolder = null;
    state.renamingPath = null;
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
