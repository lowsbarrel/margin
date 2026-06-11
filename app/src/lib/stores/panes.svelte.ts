import { IMAGE_EXTS, mimeForPath } from "$lib/utils/mime";
import {
  readFileBytes,
  watchFile,
  unwatchFile,
} from "$lib/fs/bridge";
import { files } from "$lib/stores/files.svelte";
import { editor } from "$lib/stores/editor.svelte";
import { vault } from "$lib/stores/vault.svelte";
import { toast } from "$lib/stores/toast.svelte";
import type { WorkspacePane } from "$lib/settings/workspace";
import * as m from "$lib/paraglide/messages.js";


export type TabType = "markdown" | "image" | "pdf" | "canvas" | "graph" | "unknown";

export interface Tab {
  id: number;
  path: string;
  content: string;
  type: TabType;
  blobUrl?: string;
  pdfData?: Uint8Array;
  /** Pinned tabs sort to the front of the pane and survive close-others/all. */
  pinned: boolean;
  /**
   * Last known ProseMirror caret position. Only consumed once, when the editor
   * first mounts (i.e. on workspace restore after a restart) — within a session
   * the cached editor instance already keeps its own caret/scroll.
   */
  cursorPos?: number;
}

export interface Pane {
  id: number;
  tabs: Tab[];
  activeTabIndex: number;
  externalContentVersion: number;
}

/** A recently-closed tab, reopenable via {@link panes.reopenClosedTab}. */
interface ClosedTab {
  path: string;
  type: TabType;
}

const MAX_CLOSED_HISTORY = 10;


let _nextTabId = 0;
let _nextPaneId = 0;

export function nextTabId(): number {
  return _nextTabId++;
}

export function nextPaneId(): number {
  return _nextPaneId++;
}


const PDF_EXTS = new Set([".pdf"]);

export function getTabType(path: string): TabType {
  const ext = path.slice(path.lastIndexOf(".")).toLowerCase();
  if (ext === ".md") return "markdown";
  if (ext === ".canvas") return "canvas";
  if (IMAGE_EXTS.has(ext)) return "image";
  if (PDF_EXTS.has(ext)) return "pdf";
  return "unknown";
}

export function fileTitle(path: string): string {
  if (path === "__graph__") return "Graph";
  const name = path.split("/").pop() ?? "";
  if (name.endsWith(".md")) return name.slice(0, -3);
  if (name.endsWith(".canvas")) return name.slice(0, -7);
  return name;
}

export function toBreadcrumbs(path: string, vaultPath: string | null): string[] {
  if (path === "__graph__") return ["Graph"];
  if (!vaultPath) return [];
  const rel = path.slice(vaultPath.length + 1);
  const parts = rel.split("/");
  return parts.map((p, i) => (i === parts.length - 1 ? fileTitle(path) : p));
}

export function remapPath(
  path: string,
  from: string,
  to: string,
  isDir: boolean,
): string {
  if (path === from) return to;
  if (isDir && path.startsWith(`${from}/`)) {
    return `${to}${path.slice(from.length)}`;
  }
  return path;
}

export function pathMatches(path: string, target: string, isDir: boolean): boolean {
  return path === target || (isDir && path.startsWith(`${target}/`));
}

function removeFlexAt(flexes: number[], index: number): number[] {
  const removed = flexes[index];
  const next = flexes.filter((_, i) => i !== index);
  const ni = Math.min(index > 0 ? index - 1 : 0, next.length - 1);
  if (next.length > 0) next[ni] += removed;
  return next;
}

function revokeBlobUrls(tabs: Tab[]): void {
  for (const t of tabs) {
    if (t.blobUrl) URL.revokeObjectURL(t.blobUrl);
  }
}

function createEmptyPane(): Pane {
  return {
    id: nextPaneId(),
    tabs: [],
    activeTabIndex: -1,
    externalContentVersion: 0,
  };
}


let _panes = $state<Pane[]>([createEmptyPane()]);
let _paneFlexes = $state<number[]>([1]);
let _activePaneIndex = $state(0);
let _fileSelectGeneration = 0;
let _closedTabs = $state<ClosedTab[]>([]);

function pushClosedTab(tab: Tab): void {
  // Skip transient/unsupported tabs that can't be meaningfully reopened.
  if (tab.type === "unknown") return;
  _closedTabs = [
    { path: tab.path, type: tab.type },
    ..._closedTabs.filter((t) => t.path !== tab.path).slice(0, MAX_CLOSED_HISTORY - 1),
  ];
}

/** Re-sort a pane's tabs so pinned ones lead, preserving the active tab by id. */
function applyPinOrder(paneIndex: number, tabs: Tab[], activeId: number | null) {
  const pinned = tabs.filter((t) => t.pinned);
  const unpinned = tabs.filter((t) => !t.pinned);
  const next = [...pinned, ...unpinned];
  _panes[paneIndex].tabs = next;
  const idx = activeId == null ? -1 : next.findIndex((t) => t.id === activeId);
  _panes[paneIndex].activeTabIndex =
    idx >= 0 ? idx : Math.min(_panes[paneIndex].activeTabIndex, next.length - 1);
}


export const panes = {
  get list(): Pane[] {
    return _panes;
  },
  get flexes(): number[] {
    return _paneFlexes;
  },
  get activePaneIndex(): number {
    return _activePaneIndex;
  },
  get activePane(): Pane {
    return _panes[_activePaneIndex] ?? _panes[0];
  },
  get activeTab(): Tab | null {
    const p = _panes[_activePaneIndex] ?? _panes[0];
    return p && p.activeTabIndex >= 0 && p.activeTabIndex < p.tabs.length
      ? p.tabs[p.activeTabIndex]
      : null;
  },
  get canReopenClosedTab(): boolean {
    return _closedTabs.length > 0;
  },

  set list(v: Pane[]) {
    _panes = v;
  },
  set flexes(v: number[]) {
    _paneFlexes = v;
  },
  set activePaneIndex(v: number) {
    _activePaneIndex = v;
  },


  async switchTab(paneIndex: number, tabIndex: number) {
    const pane = _panes[paneIndex];
    if (
      !pane ||
      tabIndex === pane.activeTabIndex ||
      tabIndex < 0 ||
      tabIndex >= pane.tabs.length
    )
      return;

    if (paneIndex === _activePaneIndex) await unwatchFile();

    _panes[paneIndex].activeTabIndex = tabIndex;
    const tab = _panes[paneIndex].tabs[tabIndex];

    if (paneIndex === _activePaneIndex) {
      files.setActiveFile(tab.path);
      editor.setDirty(false);
      if (vault.vaultPath) files.revealFile(tab.path, vault.vaultPath);
      if (tab.type === "markdown") await watchFile(tab.path);
    }
  },

  async closeTab(paneIndex: number, tabIndex: number) {
    const pane = _panes[paneIndex];
    const closing = pane.tabs[tabIndex];
    pushClosedTab(closing);
    revokeBlobUrls([closing]);

    const oldActiveIndex = pane.activeTabIndex;
    _panes[paneIndex].tabs = pane.tabs.filter((_, i) => i !== tabIndex);

    if (_panes[paneIndex].tabs.length === 0) {
      if (_panes.length > 1) {
        await this.closePane(paneIndex);
        return;
      }
      _panes[paneIndex].activeTabIndex = -1;
      if (paneIndex === _activePaneIndex) {
        files.setActiveFile(null);
        await unwatchFile();
      }
      return;
    }

    if (tabIndex === oldActiveIndex) {
      const newIndex = Math.min(tabIndex, _panes[paneIndex].tabs.length - 1);
      _panes[paneIndex].activeTabIndex = -1;
      await this.switchTab(paneIndex, newIndex);
    } else if (tabIndex < oldActiveIndex) {
      _panes[paneIndex].activeTabIndex = oldActiveIndex - 1;
    }
  },

  async closeOtherTabs(paneIndex: number, tabIndex: number) {
    const pane = _panes[paneIndex];
    const keepTab = pane.tabs[tabIndex];
    // Pinned tabs (other than the kept one) survive; everything else closes.
    const removed = pane.tabs.filter((t, i) => i !== tabIndex && !t.pinned);
    const kept = pane.tabs.filter((t, i) => i === tabIndex || t.pinned);
    for (const t of removed) pushClosedTab(t);
    revokeBlobUrls(removed);
    _panes[paneIndex].tabs = kept;
    _panes[paneIndex].activeTabIndex = kept.findIndex((t) => t.id === keepTab.id);
    if (paneIndex === _activePaneIndex) {
      await unwatchFile();
      files.setActiveFile(keepTab.path);
      editor.setDirty(false);
      if (keepTab.type === "markdown") await watchFile(keepTab.path);
    }
  },

  async closeAllTabs(paneIndex: number) {
    const pane = _panes[paneIndex];
    const pinned = pane.tabs.filter((t) => t.pinned);
    const removed = pane.tabs.filter((t) => !t.pinned);
    for (const t of removed) pushClosedTab(t);
    revokeBlobUrls(removed);

    // Pinned tabs keep the pane alive; activate the first of them.
    if (pinned.length > 0) {
      _panes[paneIndex].tabs = pinned;
      _panes[paneIndex].activeTabIndex = -1;
      await this.switchTab(paneIndex, 0);
      return;
    }

    if (_panes.length > 1) {
      await this.closePane(paneIndex);
      return;
    }
    _panes[paneIndex].tabs = [];
    _panes[paneIndex].activeTabIndex = -1;
    if (paneIndex === _activePaneIndex) {
      files.setActiveFile(null);
      await unwatchFile();
    }
  },

  togglePin(paneIndex: number, tabIndex: number) {
    const pane = _panes[paneIndex];
    if (!pane) return;
    const tab = pane.tabs[tabIndex];
    if (!tab) return;
    const activeId =
      pane.activeTabIndex >= 0 ? pane.tabs[pane.activeTabIndex]?.id ?? null : null;
    const next = pane.tabs.map((t) =>
      t.id === tab.id ? { ...t, pinned: !t.pinned } : t,
    );
    applyPinOrder(paneIndex, next, activeId);
  },

  async reopenClosedTab(): Promise<boolean> {
    const entry = _closedTabs[0];
    if (!entry) return false;
    _closedTabs = _closedTabs.slice(1);
    if (entry.type === "graph") {
      this.openGraph();
      return true;
    }
    return this.openFile(entry.path);
  },


  async focusPane(paneIndex: number) {
    if (paneIndex === _activePaneIndex) return;
    await unwatchFile();
    _activePaneIndex = paneIndex;
    const pane = _panes[paneIndex];
    const tab =
      pane.activeTabIndex >= 0 ? pane.tabs[pane.activeTabIndex] : null;
    files.setActiveFile(tab?.path ?? null);
    editor.setDirty(false);
    if (tab?.type === "markdown" && tab.path) await watchFile(tab.path);
  },

  async closePane(paneIndex: number) {
    if (_panes.length <= 1) return;
    revokeBlobUrls(_panes[paneIndex].tabs);
    if (paneIndex === _activePaneIndex) await unwatchFile();
    _paneFlexes = removeFlexAt(_paneFlexes, paneIndex);
    _panes = _panes.filter((_, i) => i !== paneIndex);
    const newActive = Math.min(_activePaneIndex, _panes.length - 1);
    _activePaneIndex = newActive;
    const tab =
      _panes[newActive].activeTabIndex >= 0
        ? _panes[newActive].tabs[_panes[newActive].activeTabIndex]
        : null;
    files.setActiveFile(tab?.path ?? null);
    if (tab?.type === "markdown" && tab.path) await watchFile(tab.path);
  },

  async restoreWatchingForPane(paneIndex: number) {
    const pane = _panes[paneIndex];
    if (!pane || pane.tabs.length === 0) {
      _panes[paneIndex].activeTabIndex = -1;
      if (paneIndex === _activePaneIndex) {
        files.setActiveFile(null);
        await unwatchFile();
      }
      return;
    }

    const currentActiveIndex = pane.activeTabIndex;
    const currentPath =
      currentActiveIndex >= 0 && currentActiveIndex < pane.tabs.length
        ? pane.tabs[currentActiveIndex].path
        : undefined;

    if (currentPath) {
      const index = pane.tabs.findIndex((tab) => tab.path === currentPath);
      if (index >= 0) {
        if (paneIndex === _activePaneIndex) {
          files.setActiveFile(currentPath);
          await watchFile(currentPath);
        }
        return;
      }
    }

    const fallbackIndex = Math.min(
      Math.max(currentActiveIndex, 0),
      pane.tabs.length - 1,
    );
    _panes[paneIndex].activeTabIndex = -1;
    await this.switchTab(paneIndex, fallbackIndex);
  },


  async openFile(path: string): Promise<boolean> {
    if (!vault.vaultPath) return false;
    const gen = ++_fileSelectGeneration;
    const paneIndex = _activePaneIndex;
    const pane = _panes[paneIndex];

    const tabType = getTabType(path);
    if (tabType === "unknown") {
      toast.info(m.editor_cannot_render());
      return false;
    }

    const existingIndex = pane.tabs.findIndex((t) => t.path === path);
    if (existingIndex >= 0) {
      await this.switchTab(paneIndex, existingIndex);
      return true;
    }

    files.revealFile(path, vault.vaultPath);
    files.setActiveFile(path);
    await unwatchFile();
    if (gen !== _fileSelectGeneration) return false;

    let content = "";
    let blobUrl: string | undefined;
    let pdfData: Uint8Array | undefined;
    try {
      const bytes = await readFileBytes(path);
      if (gen !== _fileSelectGeneration) return false;
      if (tabType === "markdown" || tabType === "canvas") {
        content = new TextDecoder().decode(bytes);
      } else if (tabType === "pdf") {
        pdfData = new Uint8Array(bytes);
      } else {
        const blob = new Blob([bytes.buffer as ArrayBuffer], {
          type: mimeForPath(path),
        });
        blobUrl = URL.createObjectURL(blob);
      }
    } catch (err) {
      console.warn("Failed to read file:", path, err);
      toast.error(m.toast_file_read_failed({ error: String(err) }));
      return false;
    }

    const newTab: Tab = {
      id: nextTabId(),
      path,
      content,
      type: tabType,
      blobUrl,
      pdfData,
      pinned: false,
    };
    _panes[paneIndex].tabs = [..._panes[paneIndex].tabs, newTab];
    _panes[paneIndex].activeTabIndex = _panes[paneIndex].tabs.length - 1;

    files.setActiveFile(path);
    editor.setDirty(false);

    if (tabType === "markdown") await watchFile(path);
    return true;
  },

  openGraph() {
    const paneIndex = _activePaneIndex;
    const pane = _panes[paneIndex];

    const existingIndex = pane.tabs.findIndex((t) => t.path === "__graph__");
    if (existingIndex >= 0) {
      this.switchTab(paneIndex, existingIndex);
      return;
    }

    const newTab: Tab = {
      id: nextTabId(),
      path: "__graph__",
      content: "",
      type: "graph",
      pinned: false,
    };
    _panes[paneIndex].tabs = [..._panes[paneIndex].tabs, newTab];
    _panes[paneIndex].activeTabIndex = _panes[paneIndex].tabs.length - 1;
  },

  async openFileInNewPane(
    path: string,
    refPaneIndex: number,
    side: "left" | "right",
  ) {
    const insertAt = side === "left" ? refPaneIndex : refPaneIndex + 1;
    const half = _paneFlexes[refPaneIndex] / 2;
    const newFlexes = [..._paneFlexes];
    newFlexes[refPaneIndex] = half;
    newFlexes.splice(insertAt, 0, half);
    const newPane: Pane = createEmptyPane();
    _panes = [..._panes.slice(0, insertAt), newPane, ..._panes.slice(insertAt)];
    _paneFlexes = newFlexes;
    _activePaneIndex = insertAt;
    await this.openFile(path);
  },


  async moveTabToPane(
    srcPaneIndex: number,
    srcTabIndex: number,
    destPaneIndex: number,
  ) {
    if (srcPaneIndex === destPaneIndex) return;
    const workPanes: Pane[] = _panes.map((p) => ({ ...p, tabs: [...p.tabs] }));
    const workFlexes: number[] = [..._paneFlexes];
    const tab = workPanes[srcPaneIndex].tabs[srcTabIndex];

    const srcActive = workPanes[srcPaneIndex].activeTabIndex;
    workPanes[srcPaneIndex].tabs = workPanes[srcPaneIndex].tabs.filter(
      (_, i) => i !== srcTabIndex,
    );
    if (workPanes[srcPaneIndex].tabs.length === 0)
      workPanes[srcPaneIndex].activeTabIndex = -1;
    else if (srcTabIndex === srcActive)
      workPanes[srcPaneIndex].activeTabIndex = Math.min(
        srcTabIndex,
        workPanes[srcPaneIndex].tabs.length - 1,
      );
    else if (srcTabIndex < srcActive) workPanes[srcPaneIndex].activeTabIndex--;

    workPanes[destPaneIndex].tabs = [...workPanes[destPaneIndex].tabs, tab];
    workPanes[destPaneIndex].activeTabIndex =
      workPanes[destPaneIndex].tabs.length - 1;

    let actualDest = destPaneIndex;
    if (workPanes[srcPaneIndex].tabs.length === 0) {
      if (srcPaneIndex < destPaneIndex) actualDest--;
      const removed = workFlexes[srcPaneIndex];
      workFlexes.splice(srcPaneIndex, 1);
      workPanes.splice(srcPaneIndex, 1);
      const ni = Math.min(
        srcPaneIndex > 0 ? srcPaneIndex - 1 : 0,
        workFlexes.length - 1,
      );
      if (workFlexes.length > 0) workFlexes[ni] += removed;
    }

    await unwatchFile();
    _panes = workPanes;
    _paneFlexes = workFlexes;
    _activePaneIndex = Math.min(actualDest, _panes.length - 1);
    const destTab =
      _panes[_activePaneIndex].tabs[_panes[_activePaneIndex].activeTabIndex];
    if (destTab) {
      files.setActiveFile(destTab.path);
      editor.setDirty(false);
      if (destTab.type === "markdown") await watchFile(destTab.path);
    }
  },

  async moveTabToNewPane(
    srcPaneIndex: number,
    srcTabIndex: number,
    refPaneIndex: number,
    side: "left" | "right",
  ) {
    const workPanes: Pane[] = _panes.map((p) => ({ ...p, tabs: [...p.tabs] }));
    const workFlexes: number[] = [..._paneFlexes];
    const tab = workPanes[srcPaneIndex].tabs[srcTabIndex];

    const srcActive = workPanes[srcPaneIndex].activeTabIndex;
    workPanes[srcPaneIndex].tabs = workPanes[srcPaneIndex].tabs.filter(
      (_, i) => i !== srcTabIndex,
    );
    if (workPanes[srcPaneIndex].tabs.length === 0)
      workPanes[srcPaneIndex].activeTabIndex = -1;
    else if (srcTabIndex === srcActive)
      workPanes[srcPaneIndex].activeTabIndex = Math.min(
        srcTabIndex,
        workPanes[srcPaneIndex].tabs.length - 1,
      );
    else if (srcTabIndex < srcActive) workPanes[srcPaneIndex].activeTabIndex--;

    let adjustedRef = refPaneIndex;
    if (workPanes[srcPaneIndex].tabs.length === 0) {
      if (srcPaneIndex < refPaneIndex) adjustedRef--;
      const srcFlex = workFlexes[srcPaneIndex];
      workFlexes.splice(srcPaneIndex, 1);
      workPanes.splice(srcPaneIndex, 1);
      const safeRef = Math.min(adjustedRef, workFlexes.length - 1);
      if (workFlexes.length > 0) workFlexes[safeRef] += srcFlex;
    }

    const insertAt = side === "left" ? adjustedRef : adjustedRef + 1;
    const half = workFlexes[adjustedRef] / 2;
    workFlexes[adjustedRef] = half;
    workFlexes.splice(insertAt, 0, half);
    const newPane: Pane = {
      id: nextPaneId(),
      tabs: [tab],
      activeTabIndex: 0,
      externalContentVersion: 0,
    };
    workPanes.splice(insertAt, 0, newPane);

    await unwatchFile();
    _panes = workPanes;
    _paneFlexes = workFlexes;
    _activePaneIndex = insertAt;
    files.setActiveFile(tab.path);
    editor.setDirty(false);
    if (tab.type === "markdown") await watchFile(tab.path);
  },


  remapPaths(from: string, to: string, isDir: boolean) {
    for (let pi = 0; pi < _panes.length; pi++) {
      _panes[pi].tabs = _panes[pi].tabs.map((tab) => ({
        ...tab,
        path: remapPath(tab.path, from, to, isDir),
      }));
    }
  },

  removePaths(path: string, isDir: boolean) {
    for (let pi = 0; pi < _panes.length; pi++) {
      _panes[pi].tabs = _panes[pi].tabs.filter(
        (tab) => !pathMatches(tab.path, path, isDir),
      );
    }
  },


  /**
   * Replace the content of the active tab when an external change is detected
   * for `path`. Returns true if a matching tab was updated. Keeps the mutation
   * (including the `externalContentVersion` bump that forces the editor to
   * reload) inside the store instead of reaching into nested `$state`.
   */
  applyExternalContent(path: string, content: string): boolean {
    const pi = _activePaneIndex;
    const pane = _panes[pi];
    if (!pane) return false;
    const ti = pane.activeTabIndex;
    if (ti < 0 || ti >= pane.tabs.length) return false;
    if (pane.tabs[ti].path !== path) return false;
    if (pane.tabs[ti].content === content) return false;
    _panes[pi].tabs[ti] = { ..._panes[pi].tabs[ti], content };
    _panes[pi].externalContentVersion++;
    return true;
  },

  broadcastContent(
    sourcePaneIndex: number,
    filePath: string,
    content: string,
  ) {
    // Only relevant when the same file is open in another pane.
    if (_panes.length < 2) return;
    for (let pi = 0; pi < _panes.length; pi++) {
      if (pi === sourcePaneIndex) continue;
      const pane = _panes[pi];
      if (!pane.tabs.some((t) => t.path === filePath)) continue;
      _panes[pi] = {
        ...pane,
        tabs: pane.tabs.map((t) =>
          t.path === filePath ? { ...t, content } : t,
        ),
        externalContentVersion: pane.externalContentVersion + 1,
      };
    }
  },


  reset() {
    _panes.forEach((pane) => revokeBlobUrls(pane.tabs));
    _panes = [createEmptyPane()];
    _paneFlexes = [1];
    _activePaneIndex = 0;
    _closedTabs = [];
  },


  async restoreFromWorkspace(
    wsPanes: WorkspacePane[],
    wsFlexes: number[],
    wsActivePaneIndex: number,
  ) {
    // Build each tab independently; the file reads are mutually independent, so
    // run them concurrently (per pane) instead of blocking on a serial chain at
    // startup. A failed read or unsupported type yields null and is dropped.
    const buildTab = async (
      wsTab: WorkspacePane["tabs"][number],
    ): Promise<Tab | null> => {
      const path = wsTab.path;
      const tabType = getTabType(path);
      if (tabType === "unknown" && path !== "__graph__") return null;
      const type = path === "__graph__" ? ("graph" as TabType) : tabType;

      let content = "";
      let blobUrl: string | undefined;
      let pdfData: Uint8Array | undefined;
      try {
        if (path !== "__graph__") {
          const bytes = await readFileBytes(path);
          if (type === "markdown" || type === "canvas") {
            content = new TextDecoder().decode(bytes);
          } else if (type === "pdf") {
            pdfData = new Uint8Array(bytes);
          } else {
            const blob = new Blob([bytes.buffer as ArrayBuffer], { type: mimeForPath(path) });
            blobUrl = URL.createObjectURL(blob);
          }
        }
      } catch {
        return null;
      }
      return {
        id: nextTabId(),
        path,
        content,
        type,
        blobUrl,
        pdfData,
        pinned: wsTab.pinned ?? false,
        cursorPos: wsTab.cursor_pos ?? undefined,
      };
    };

    const restoredPanes: Pane[] = [];
    for (const wsPane of wsPanes) {
      const built = await Promise.all(wsPane.tabs.map((t) => buildTab(t)));
      const tabs = built.filter((t): t is Tab => t !== null);

      if (tabs.length > 0) {
        const activeIdx = Math.min(Math.max(wsPane.active_tab_index, 0), tabs.length - 1);
        restoredPanes.push({
          id: nextPaneId(),
          tabs,
          activeTabIndex: activeIdx,
          externalContentVersion: 0,
        });
      }
    }

    if (restoredPanes.length === 0) return;

    _panes = restoredPanes;
    _paneFlexes =
      wsFlexes.length === restoredPanes.length
        ? wsFlexes
        : restoredPanes.map(() => 1);
    _activePaneIndex = Math.min(wsActivePaneIndex, restoredPanes.length - 1);

    const ap = _panes[_activePaneIndex];
    const at =
      ap.activeTabIndex >= 0 && ap.activeTabIndex < ap.tabs.length
        ? ap.tabs[ap.activeTabIndex]
        : null;
    if (at) {
      files.setActiveFile(at.path);
      if (at.type === "markdown") await watchFile(at.path);
    }
  },
};
