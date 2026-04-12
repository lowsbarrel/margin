import { buildVisibleTree, type TreeEntry } from "$lib/fs/bridge";

export type SortOrder = "name" | "date";
// Re-export for consumers that need the type
export type { TreeEntry };

export interface SelectedEntry {
  path: string;
  isDir: boolean;
}

interface FilesState {
  /** Flat, sorted, depth-annotated list of all currently-visible tree rows.
   *  Built by Rust in one call — replaces the old per-FolderNode listDirectory pattern. */
  flatTree: TreeEntry[];
  /** Cached vault root used by reactive rebuilds. */
  vaultRoot: string | null;
  activeFile: string | null;
  selectedFolder: string | null;
  /** Currently selected entries in the file tree (for clipboard ops & multi-select). */
  selectedEntries: Map<string, SelectedEntry>;
  /** The last clicked path — anchor for Shift+click range selection. */
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

/** Internal: ask Rust to rebuild the flat tree for the current expanded set. */
async function _rebuild(): Promise<void> {
  if (!state.vaultRoot) return;
  const expanded = [...state.expandedFolders];
  const flatTree = await buildVisibleTree(
    state.vaultRoot,
    expanded,
    state.sortOrder,
  );
  state.flatTree = flatTree;
  // Rebuild path→index lookup for O(1) range selection
  _pathIndex = new Map(flatTree.map((r, i) => [r.path, i]));
}

/** Cached path→index map — rebuilt alongside flatTree. */
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

  /** Expand all ancestor folders of a file path so it's visible in the tree. */
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

  // ─── Multi-selection ────────────────────────────────────────────────────

  get selectedEntries() {
    return state.selectedEntries;
  },

  /** Convenience: the single selected entry when exactly one is selected. */
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

  /** Plain click — clear selection, select only this entry. */
  selectSingle(path: string, isDir: boolean) {
    state.selectedEntries = new Map([[path, { path, isDir }]]);
    state.lastSelectedPath = path;
  },

  /** Cmd/Ctrl+click — toggle one entry in or out of the selection. */
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

  /** Shift+click — range select from lastSelectedPath to this path. */
  selectRange(path: string) {
    const anchor = state.lastSelectedPath;
    if (!anchor) {
      // No anchor — just select the clicked item
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
    // Don't update lastSelectedPath — keep the anchor
  },

  /** Check if a path is selected. */
  isSelected(path: string) {
    return state.selectedEntries.has(path);
  },

  /** Get all selected entries as arrays. */
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

  /** Select all visible entries. */
  selectAll() {
    const next = new Map<string, SelectedEntry>();
    for (const row of state.flatTree) {
      next.set(row.path, { path: row.path, isDir: row.is_dir });
    }
    state.selectedEntries = next;
  },

  /** Legacy compat for setSelectedEntry calls. */
  setSelectedEntry(path: string, isDir: boolean) {
    this.selectSingle(path, isDir);
  },

  async expandFolder(path: string) {
    state.expandedFolders.add(path);
    state.expandedFolders = new Set(state.expandedFolders);
    await _rebuild();
  },

  collapseAll() {
    state.expandedFolders = new Set();
    _rebuild();
  },

  async collapseFolder(path: string) {
    state.expandedFolders.delete(path);
    state.expandedFolders = new Set(state.expandedFolders);
    await _rebuild();
  },

  async toggleFolder(path: string) {
    if (state.expandedFolders.has(path)) {
      state.expandedFolders.delete(path);
    } else {
      state.expandedFolders.add(path);
    }
    state.expandedFolders = new Set(state.expandedFolders);
    await _rebuild();
  },

  /** Rebuild the visible tree. Pass vaultPath on first call or vault change. */
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
