<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { files } from "$lib/stores/files.svelte";
  import { favourites } from "$lib/stores/favourites.svelte";
  import { editor } from "$lib/stores/editor.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import Login from "$lib/components/Login.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Editor from "$lib/components/Editor.svelte";
  import ImageViewer from "$lib/components/ImageViewer.svelte";
  import PdfViewer from "$lib/components/PdfViewer.svelte";
  import CanvasEditor from "$lib/components/CanvasEditor.svelte";
  import GraphView from "$lib/components/GraphView.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import HistoryPanel from "$lib/components/HistoryPanel.svelte";
  import {
    deleteEntry,
    readFileBytes,
    watchFile,
    unwatchFile,
    onFileChanged,
    watchVault,
    unwatchVault,
    onVaultFsChanged,
    hasUnsyncedChanges,
    renameEntry,
    searchFiles,
  } from "$lib/fs/bridge";
  import { clearHistoryTree, renameHistory } from "$lib/history/bridge";
  import { loadSettings } from "$lib/settings/bridge";
  import { s3Configure } from "$lib/s3/bridge";
  import {
    startAutoSync,
    stopAutoSync,
    runQuietSync,
    setSyncCredentials,
    clearSyncCredentials,
    type ConflictStrategy,
  } from "$lib/sync/s3sync";
  import { Toaster } from "$lib/ui";
  import { X, ChevronRight } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";
  import { drag } from "$lib/stores/drag.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import type { ContextMenuItem } from "$lib/components/ContextMenu.svelte";
  import QuickSwitcher from "$lib/components/QuickSwitcher.svelte";
  import { mimeForPath } from "$lib/utils/mime";
  import { checkForAppUpdate } from "$lib/utils/updater";
  import {
    type TabType,
    type Tab,
    type Pane,
    nextTabId,
    nextPaneId,
    getTabType,
    fileTitle,
    toBreadcrumbs,
    remapPath,
    pathMatches,
    removeFlexAt,
    revokeBlobUrls,
    broadcastContent,
    createEmptyPane,
  } from "$lib/stores/panes.svelte";

  let panes = $state<Pane[]>([createEmptyPane()]);
  let paneFlexes = $state<number[]>([1]);
  let activePaneIndex = $state(0);
  let showSettings = $state(false);
  let showQuickSwitcher = $state(false);
  let showHistory = $state(false);
  let sidebarOpen = $state(true);
  let sidebarFocusSearch = $state(false);
  let attachmentFolder = $state<string | null>(null);
  let lastSaveTime = 0;
  let unlistenFileChange: (() => void) | null = null;
  let unlistenVaultChange: (() => void) | null = null;
  let updateInterval: ReturnType<typeof setInterval> | null = null;
  let panesContainerEl = $state<HTMLElement | null>(null);
  let dividerResizing = $state(false);

  // Drag & drop
  let dropTarget = $state<{
    paneIndex: number;
    zone: "left" | "center" | "right";
  } | null>(null);
  let tabContextMenu = $state<{
    x: number;
    y: number;
    paneIndex: number;
    tabIndex: number;
  } | null>(null);

  let activePane = $derived(panes[activePaneIndex] ?? panes[0]);
  let activeTab = $derived(
    activePane &&
      activePane.activeTabIndex >= 0 &&
      activePane.activeTabIndex < activePane.tabs.length
      ? activePane.tabs[activePane.activeTabIndex]
      : null,
  );



  async function restoreWatchingForPane(paneIndex: number) {
    const pane = panes[paneIndex];
    if (!pane || pane.tabs.length === 0) {
      panes[paneIndex].activeTabIndex = -1;
      if (paneIndex === activePaneIndex) {
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
        if (paneIndex === activePaneIndex) {
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
    panes[paneIndex].activeTabIndex = -1;
    await switchTab(paneIndex, fallbackIndex);
  }

  async function handleRename(oldPath: string, newPath: string, isDir = false) {
    if (!vault.vaultPath || oldPath === newPath) return;
    try {
      await unwatchFile();
      // Rename history first so it stays attached to the file.
      // If the file rename subsequently fails, revert the history rename.
      let historyRenamed = false;
      try {
        await renameHistory(vault.vaultPath, oldPath, newPath);
        historyRenamed = true;
      } catch (err) {
        console.warn("Failed to rename history:", err);
      }

      try {
        await renameEntry(oldPath, newPath);
      } catch (err) {
        // Revert history rename if the file rename failed
        if (historyRenamed) {
          renameHistory(vault.vaultPath, newPath, oldPath).catch((revertErr) =>
            console.warn("Failed to revert history rename:", revertErr),
          );
        }
        throw err;
      }

      for (let pi = 0; pi < panes.length; pi++) {
        panes[pi].tabs = panes[pi].tabs.map((tab) => ({
          ...tab,
          path: remapPath(tab.path, oldPath, newPath, isDir),
        }));
      }

      if (files.activeFile) {
        files.setActiveFile(
          remapPath(files.activeFile, oldPath, newPath, isDir),
        );
      }
      if (files.selectedFolder) {
        files.setSelectedFolder(
          remapPath(files.selectedFolder, oldPath, newPath, isDir),
        );
      }
      favourites.renamePath(oldPath, newPath);
      await files.refresh(vault.vaultPath);
      await restoreWatchingForPane(activePaneIndex);
    } catch (err) {
      console.error("Rename failed:", err);
      throw err;
    }
  }

  async function handleDelete(path: string, isDir: boolean) {
    if (!vault.vaultPath) return;
    try {
      await unwatchFile();

      for (let pi = 0; pi < panes.length; pi++) {
        panes[pi].tabs = panes[pi].tabs.filter(
          (tab) => !pathMatches(tab.path, path, isDir),
        );
      }

      if (files.activeFile && pathMatches(files.activeFile, path, isDir)) {
        files.setActiveFile(null);
      }
      if (
        files.selectedFolder &&
        pathMatches(files.selectedFolder, path, isDir)
      ) {
        files.setSelectedFolder(vault.vaultPath);
      }
      // Clear history before deleting the file so history doesn't get orphaned
      // if deleteEntry fails. If clearHistoryTree fails, the file still exists
      // and history will reattach naturally.
      try {
        await clearHistoryTree(vault.vaultPath, path);
      } catch (err) {
        console.warn("Failed to clear history for deleted entry:", err);
      }
      await deleteEntry(path);
      favourites.removePath(path);
      await files.refresh(vault.vaultPath);
      await restoreWatchingForPane(activePaneIndex);
    } catch (err) {
      console.error("Delete failed:", err);
      throw err;
    }
  }

  async function handleWikiLink(title: string) {
    if (!vault.vaultPath) return;
    const results = await searchFiles(vault.vaultPath, title);
    const match = results.find(
      (r) =>
        !r.is_dir && (r.name === `${title}.md` || r.name === `${title}.canvas`),
    );
    if (match) {
      await handleFileSelect(match.path);
    } else {
      toast.info(`Note not found: ${title}`);
    }
  }

  function handleOpenGraph() {
    const paneIndex = activePaneIndex;
    const pane = panes[paneIndex];

    // If a graph tab already exists in this pane, switch to it
    const existingIndex = pane.tabs.findIndex((t) => t.path === "__graph__");
    if (existingIndex >= 0) {
      switchTab(paneIndex, existingIndex);
      return;
    }

    const newTab: Tab = {
      id: nextTabId(),
      path: "__graph__",
      content: "",
      type: "graph",
    };
    panes[paneIndex].tabs = [...panes[paneIndex].tabs, newTab];
    panes[paneIndex].activeTabIndex = panes[paneIndex].tabs.length - 1;
  }

  async function handleFileSelect(path: string) {
    if (!vault.vaultPath) return;
    const paneIndex = activePaneIndex;
    const pane = panes[paneIndex];

    const tabType = getTabType(path);
    if (tabType === "unknown") {
      toast.info(m.editor_cannot_render());
      return;
    }

    const existingIndex = pane.tabs.findIndex((t) => t.path === path);
    if (existingIndex >= 0) {
      await switchTab(paneIndex, existingIndex);
      return;
    }

    files.revealFile(path, vault.vaultPath);
    await unwatchFile();

    let content = "";
    let blobUrl: string | undefined;
    let pdfData: Uint8Array | undefined;
    try {
      const bytes = await readFileBytes(path);
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
      return;
    }

    const newTab: Tab = {
      id: nextTabId(),
      path,
      content,
      type: tabType,
      blobUrl,
      pdfData,
    };
    panes[paneIndex].tabs = [...panes[paneIndex].tabs, newTab];
    panes[paneIndex].activeTabIndex = panes[paneIndex].tabs.length - 1;

    files.setActiveFile(path);
    editor.setDirty(false);

    if (tabType === "markdown") {
      await watchFile(path);
    }
  }

  async function switchTab(paneIndex: number, tabIndex: number) {
    const pane = panes[paneIndex];
    if (
      !pane ||
      tabIndex === pane.activeTabIndex ||
      tabIndex < 0 ||
      tabIndex >= pane.tabs.length
    )
      return;

    if (paneIndex === activePaneIndex) {
      await unwatchFile();
    }

    panes[paneIndex].activeTabIndex = tabIndex;
    const tab = panes[paneIndex].tabs[tabIndex];

    if (paneIndex === activePaneIndex) {
      files.setActiveFile(tab.path);
      editor.setDirty(false);
      if (vault.vaultPath) {
        files.revealFile(tab.path, vault.vaultPath);
      }
      if (tab.type === "markdown") {
        await watchFile(tab.path);
      }
    }
  }

  async function closeOtherTabs(paneIndex: number, tabIndex: number) {
    const pane = panes[paneIndex];
    const keepTab = pane.tabs[tabIndex];
    revokeBlobUrls(pane.tabs.filter((_, i) => i !== tabIndex));
    panes[paneIndex].tabs = [keepTab];
    panes[paneIndex].activeTabIndex = 0;
    if (paneIndex === activePaneIndex) {
      await unwatchFile();
      files.setActiveFile(keepTab.path);
      editor.setDirty(false);
      if (keepTab.type === "markdown") await watchFile(keepTab.path);
    }
  }

  async function closeAllTabs(paneIndex: number) {
    const pane = panes[paneIndex];
    revokeBlobUrls(pane.tabs);
    if (panes.length > 1) {
      await closePane(paneIndex);
      return;
    }
    panes[paneIndex].tabs = [];
    panes[paneIndex].activeTabIndex = -1;
    if (paneIndex === activePaneIndex) {
      files.setActiveFile(null);
      await unwatchFile();
    }
  }

  function handleTabContextMenu(
    e: MouseEvent,
    paneIndex: number,
    tabIndex: number,
  ) {
    e.preventDefault();
    e.stopPropagation();
    tabContextMenu = { x: e.clientX, y: e.clientY, paneIndex, tabIndex };
  }

  function getTabContextMenuItems(
    paneIndex: number,
    tabIndex: number,
  ): ContextMenuItem[] {
    const pane = panes[paneIndex];
    return [
      { label: m.tab_close(), onclick: () => closeTab(paneIndex, tabIndex) },
      {
        label: m.tab_close_others(),
        onclick: () => closeOtherTabs(paneIndex, tabIndex),
        disabled: pane.tabs.length <= 1,
      },
      { label: m.tab_close_all(), onclick: () => closeAllTabs(paneIndex) },
    ];
  }

  async function closeTab(paneIndex: number, tabIndex: number) {
    const pane = panes[paneIndex];
    const closing = pane.tabs[tabIndex];
    revokeBlobUrls([closing]);

    const oldActiveIndex = pane.activeTabIndex;
    panes[paneIndex].tabs = pane.tabs.filter((_, i) => i !== tabIndex);

    if (panes[paneIndex].tabs.length === 0) {
      if (panes.length > 1) {
        await closePane(paneIndex);
        return;
      }
      panes[paneIndex].activeTabIndex = -1;
      if (paneIndex === activePaneIndex) {
        files.setActiveFile(null);
        await unwatchFile();
      }
      return;
    }

    if (tabIndex === oldActiveIndex) {
      const newIndex = Math.min(tabIndex, panes[paneIndex].tabs.length - 1);
      panes[paneIndex].activeTabIndex = -1;
      await switchTab(paneIndex, newIndex);
    } else if (tabIndex < oldActiveIndex) {
      panes[paneIndex].activeTabIndex = oldActiveIndex - 1;
    }
  }

  async function focusPane(paneIndex: number) {
    if (paneIndex === activePaneIndex) return;
    await unwatchFile();
    activePaneIndex = paneIndex;
    const pane = panes[paneIndex];
    const tab =
      pane.activeTabIndex >= 0 ? pane.tabs[pane.activeTabIndex] : null;
    files.setActiveFile(tab?.path ?? null);
    editor.setDirty(false);
    if (tab?.type === "markdown" && tab.path) {
      await watchFile(tab.path);
    }
  }

  async function closePane(paneIndex: number) {
    if (panes.length <= 1) return;
    revokeBlobUrls(panes[paneIndex].tabs);
    if (paneIndex === activePaneIndex) await unwatchFile();
    paneFlexes = removeFlexAt(paneFlexes, paneIndex);
    panes = panes.filter((_, i) => i !== paneIndex);
    const newActive = Math.min(activePaneIndex, panes.length - 1);
    activePaneIndex = newActive;
    const tab =
      panes[newActive].activeTabIndex >= 0
        ? panes[newActive].tabs[panes[newActive].activeTabIndex]
        : null;
    files.setActiveFile(tab?.path ?? null);
    if (tab?.type === "markdown" && tab.path) await watchFile(tab.path);
  }

  // ── Divider resize ────────────────────────────────────────────────────────────
  function startDividerDrag(e: MouseEvent, dividerIndex: number) {
    e.preventDefault();
    dividerResizing = true;
    if (!panesContainerEl) return;
    const startX = e.clientX;
    const containerWidth = panesContainerEl.getBoundingClientRect().width;
    const availableWidth = containerWidth - (panes.length - 1) * 4;
    const totalFlex = paneFlexes.reduce((a, b) => a + b, 0);
    const leftStartPx = (paneFlexes[dividerIndex] / totalFlex) * availableWidth;
    const rightStartPx =
      (paneFlexes[dividerIndex + 1] / totalFlex) * availableWidth;
    const minPx = 120;

    function onMove(ev: MouseEvent) {
      const delta = ev.clientX - startX;
      const leftPx = Math.max(
        minPx,
        Math.min(leftStartPx + delta, leftStartPx + rightStartPx - minPx),
      );
      const rightPx = leftStartPx + rightStartPx - leftPx;
      paneFlexes = paneFlexes.map((f, i) => {
        if (i === dividerIndex) return (leftPx / availableWidth) * totalFlex;
        if (i === dividerIndex + 1)
          return (rightPx / availableWidth) * totalFlex;
        return f;
      });
    }
    function onUp() {
      dividerResizing = false;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  // ── Pointer-based drag & drop ─────────────────────────────────────────────────

  async function openFileInNewPane(
    path: string,
    refPaneIndex: number,
    side: "left" | "right",
  ) {
    const insertAt = side === "left" ? refPaneIndex : refPaneIndex + 1;
    const half = paneFlexes[refPaneIndex] / 2;
    const newFlexes = [...paneFlexes];
    newFlexes[refPaneIndex] = half;
    newFlexes.splice(insertAt, 0, half);
    const newPane: Pane = createEmptyPane();
    panes = [...panes.slice(0, insertAt), newPane, ...panes.slice(insertAt)];
    paneFlexes = newFlexes;
    activePaneIndex = insertAt;
    await handleFileSelect(path);
  }

  function handleTabMouseDown(
    e: MouseEvent,
    paneIndex: number,
    tabIndex: number,
  ) {
    if (e.button !== 0) return;
    if (panes.length === 1 && panes[0].tabs.length === 1) return;
    e.preventDefault();
    const startX = e.clientX,
      startY = e.clientY;
    let didDrag = false;
    const label = fileTitle(panes[paneIndex].tabs[tabIndex].path);

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
      if (!didDrag) switchTab(paneIndex, tabIndex);
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  async function executeDrop(target: {
    paneIndex: number;
    zone: "left" | "center" | "right";
  }) {
    const item = drag.item;
    if (!item) return;
    if (item.kind === "file") {
      // Directories can only be dropped on sidebar folders, not panes
      if (item.isDir) return;
      if (target.zone === "center") {
        if (target.paneIndex !== activePaneIndex)
          await focusPane(target.paneIndex);
        await handleFileSelect(item.path);
      } else {
        await openFileInNewPane(item.path, target.paneIndex, target.zone);
      }
    } else if (item.kind === "tab") {
      const { paneIndex: srcPane, tabIndex: srcTab } = item;
      if (target.zone === "center")
        await moveTabToPane(srcPane, srcTab, target.paneIndex);
      else
        await moveTabToNewPane(srcPane, srcTab, target.paneIndex, target.zone);
    }
  }

  async function moveTabToPane(
    srcPaneIndex: number,
    srcTabIndex: number,
    destPaneIndex: number,
  ) {
    if (srcPaneIndex === destPaneIndex) return;
    const workPanes: Pane[] = panes.map((p) => ({ ...p, tabs: [...p.tabs] }));
    const workFlexes: number[] = [...paneFlexes];
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
    panes = workPanes;
    paneFlexes = workFlexes;
    activePaneIndex = Math.min(actualDest, panes.length - 1);
    const destTab =
      panes[activePaneIndex].tabs[panes[activePaneIndex].activeTabIndex];
    if (destTab) {
      files.setActiveFile(destTab.path);
      editor.setDirty(false);
      if (destTab.type === "markdown") await watchFile(destTab.path);
    }
  }

  async function moveTabToNewPane(
    srcPaneIndex: number,
    srcTabIndex: number,
    refPaneIndex: number,
    side: "left" | "right",
  ) {
    const workPanes: Pane[] = panes.map((p) => ({ ...p, tabs: [...p.tabs] }));
    const workFlexes: number[] = [...paneFlexes];
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
    panes = workPanes;
    paneFlexes = workFlexes;
    activePaneIndex = insertAt;
    files.setActiveFile(tab.path);
    editor.setDirty(false);
    if (tab.type === "markdown") await watchFile(tab.path);
  }

  function handleLogout() {
    stopAutoSync();
    clearSyncCredentials();
    panes.forEach((pane) => revokeBlobUrls(pane.tabs));
    panes = [createEmptyPane()];
    paneFlexes = [1];
    activePaneIndex = 0;
    files.clear();
    unwatchFile();
    unwatchVault();
    editor.setSyncStatus("idle");
    editor.setDirty(false);
    vault.lock();
  }

  function handleSwitchVault() {
    handleLogout();
  }

  $effect(() => {
    if (!editor.dirty) {
      lastSaveTime = Date.now();
    }
  });

  onMount(() => {
    // Check for OTA updates on launch, then every 5 minutes
    checkForAppUpdate();
    updateInterval = setInterval(checkForAppUpdate, 5 * 60 * 1000);

    onFileChanged(async () => {
      if (Date.now() - lastSaveTime < 2000) return;
      const tab = activeTab;
      if (!tab) return;
      try {
        const bytes = await readFileBytes(tab.path);
        const newContent = new TextDecoder().decode(bytes);
        if (newContent !== tab.content) {
          const pi = activePaneIndex;
          const ti = panes[pi].activeTabIndex;
          panes[pi].tabs[ti] = { ...panes[pi].tabs[ti], content: newContent };
          panes[pi].externalContentVersion++;
          editor.markLocalChange();
        }
      } catch (err) {
        console.warn("File may have been deleted:", err);
      }
    }).then((unlisten) => {
      unlistenFileChange = unlisten;
    });

    // Listen for any file change in the vault (external programs, new files, deletions)
    onVaultFsChanged(() => {
      editor.markLocalChange();
    }).then((unlisten) => {
      unlistenVaultChange = unlisten;
    });
  });

  onDestroy(() => {
    if (updateInterval) clearInterval(updateInterval);
    stopAutoSync();
    clearSyncCredentials();
    unlistenFileChange?.();
    unlistenVaultChange?.();
    unwatchFile();
    unwatchVault();
  });

  $effect(() => {
    if (vault.isUnlocked && vault.vaultPath) {
      const currentVaultPath = vault.vaultPath;
      const currentKey = vault.encryptionKey;
      files.refresh(currentVaultPath);
      favourites.load();
      // Start watching the entire vault for external changes
      watchVault(currentVaultPath).catch((err) =>
        console.warn("Failed to start vault watcher:", err),
      );
      if (currentKey) {
        loadSettings(currentVaultPath, currentKey)
          .then((settings) => {
            if (vault.vaultPath !== currentVaultPath) return;
            if (settings?.s3) {
              s3Configure(settings.s3);

              // Always store sync credentials so the manual sync button works
              if (vault.vaultId && vault.encryptionKey) {
                const syncOpts = {
                  conflictStrategy:
                    (settings?.conflict_strategy as ConflictStrategy) ??
                    "local_wins",
                };
                setSyncCredentials(
                  currentVaultPath,
                  vault.vaultId,
                  vault.encryptionKey,
                  settings.s3,
                  syncOpts,
                );
              }

              // Check if vault has unsynced local changes
              if (currentKey) {
                hasUnsyncedChanges(currentVaultPath, currentKey)
                  .then((pending) => {
                    if (vault.vaultPath !== currentVaultPath) return;
                    // Don't override if a sync is already in flight
                    if (editor.syncStatus === "syncing") return;
                    editor.setSyncStatus(pending ? "idle" : "synced");
                  })
                  .catch(() => {
                    if (editor.syncStatus === "syncing") return;
                    // If check fails, assume needs sync
                    editor.setSyncStatus("idle");
                  });
              }

              // Start auto-sync if enabled and S3 is configured
              if (settings.auto_sync && vault.vaultId && vault.encryptionKey) {
                const syncOpts = {
                  conflictStrategy:
                    (settings?.conflict_strategy as ConflictStrategy) ??
                    "local_wins",
                };
                startAutoSync(
                  currentVaultPath,
                  vault.vaultId,
                  vault.encryptionKey,
                  settings.s3,
                  undefined,
                  syncOpts,
                );
              }
            } else {
              editor.setSyncStatus("idle");
            }
            attachmentFolder = settings?.attachment_folder ?? null;
          })
          .catch((err) => {
            console.warn("Failed to load settings:", err);
          });
      }
    }
  });
</script>

{#if vault.isUnlocked}
  <div class="app-layout">
    <div class="main-area">
      <Sidebar
        onfileselect={handleFileSelect}
        onrenameentry={(from, to, isDir) => handleRename(from, to, isDir)}
        ondeleteentry={(path, isDir) => handleDelete(path, isDir)}
        onopengraph={handleOpenGraph}
        panelOpen={sidebarOpen}
        ontoggle={() => (sidebarOpen = !sidebarOpen)}
        bind:focusSearch={sidebarFocusSearch}
      />

      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="panes-container"
        class:resizing={dividerResizing}
        bind:this={panesContainerEl}
      >
        {#each panes as pane, paneIndex (pane.id)}
          {#if paneIndex > 0}
            <div
              class="pane-divider"
              onmousedown={(e) => startDividerDrag(e, paneIndex - 1)}
            ></div>
          {/if}
          {@const paneActiveTab =
            pane.activeTabIndex >= 0 && pane.activeTabIndex < pane.tabs.length
              ? pane.tabs[pane.activeTabIndex]
              : null}
          {@const paneCrumbs = paneActiveTab
            ? toBreadcrumbs(paneActiveTab.path, vault.vaultPath)
            : []}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="pane"
            class:pane-active={paneIndex === activePaneIndex}
            style="flex: {paneFlexes[paneIndex]}"
            onmousedown={() => focusPane(paneIndex)}
          >
            <!-- Tab Bar -->
            <div class="tab-bar">
              <div class="tabs-scroll">
                {#each pane.tabs as tab, i (tab.id)}
                  <div
                    class="tab"
                    class:active={i === pane.activeTabIndex}
                    role="tab"
                    tabindex={0}
                    aria-selected={i === pane.activeTabIndex}
                    onmousedown={(e) => {
                      e.stopPropagation();
                      handleTabMouseDown(e, paneIndex, i);
                    }}
                    oncontextmenu={(e) => handleTabContextMenu(e, paneIndex, i)}
                    onkeydown={(e) => {
                      if (e.key === "Enter" || e.key === " ")
                        switchTab(paneIndex, i);
                    }}
                  >
                    <span class="tab-label">{fileTitle(tab.path)}</span>
                    <button
                      class="tab-close"
                      onclick={(e) => {
                        e.stopPropagation();
                        closeTab(paneIndex, i);
                      }}
                      tabindex={-1}
                      aria-label={m.tab_close_label()}
                    >
                      <X size={12} />
                    </button>
                  </div>
                {/each}
              </div>
              {#if panes.length > 1}
                <div class="pane-controls">
                  <button
                    class="pane-btn"
                    onclick={(e) => {
                      e.stopPropagation();
                      closePane(paneIndex);
                    }}
                    title={m.pane_close()}
                  >
                    <X size={14} />
                  </button>
                </div>
              {/if}
            </div>

            <!-- Breadcrumbs -->
            {#if paneActiveTab && paneCrumbs.length > 0}
              <div class="breadcrumbs">
                {#each paneCrumbs as crumb, i}
                  {#if i > 0}
                    <ChevronRight size={12} />
                  {/if}
                  <span
                    class="crumb"
                    class:current={i === paneCrumbs.length - 1}>{crumb}</span
                  >
                {/each}
              </div>
            {/if}

            <!-- Editor / Viewer -->
            <main class="editor-area">
              {#if paneActiveTab}
                <!-- Markdown editors: keep all alive, hide inactive via CSS to
                     avoid destroying/recreating expensive TipTap instances on
                     every tab switch. -->
                {#each pane.tabs.filter(t => t.type === "markdown") as tab (tab.id)}
                  {@const isActive = tab.id === paneActiveTab?.id}
                  <div class="cached-editor-slot" class:cached-hidden={!isActive}>
                    <Editor
                      filePath={tab.path}
                      initialContent={tab.content}
                      externalContentVersion={pane.externalContentVersion}
                      title={fileTitle(tab.path)}
                      active={isActive && paneIndex === activePaneIndex}
                      onrename={handleRename}
                      onwikilink={handleWikiLink}
                      onsave={(content) => {
                        tab.content = content;
                        broadcastContent(
                          panes,
                          paneIndex,
                          tab.path,
                          content,
                        );
                      }}
                      {attachmentFolder}
                    />
                  </div>
                {/each}
                <!-- Non-markdown tabs: use {#key} since they're lightweight -->
                {#if paneActiveTab.type === "image" && paneActiveTab.blobUrl}
                  {#key paneActiveTab.id}
                    <ImageViewer
                      src={paneActiveTab.blobUrl}
                      alt={fileTitle(paneActiveTab.path)}
                    />
                  {/key}
                {:else if paneActiveTab.type === "pdf" && paneActiveTab.pdfData}
                  {#key paneActiveTab.id}
                    <PdfViewer data={paneActiveTab.pdfData} />
                  {/key}
                {:else if paneActiveTab.type === "canvas"}
                  {#key paneActiveTab.id}
                    <CanvasEditor
                      filePath={paneActiveTab.path}
                      initialData={paneActiveTab.content}
                      onsave={(content) => {
                        paneActiveTab.content = content;
                        broadcastContent(
                          panes,
                          paneIndex,
                          paneActiveTab.path,
                          content,
                        );
                      }}
                    />
                  {/key}
                {:else if paneActiveTab.type === "graph"}
                  {#key paneActiveTab.id}
                    <GraphView onfileselect={handleFileSelect} />
                  {/key}
                {/if}
              {:else}
                <div class="empty-state">
                  <p class="empty-text">{m.editor_empty_state()}</p>
                </div>
              {/if}
            </main>

            <!-- Drop overlay (pointer-based) -->
            {#if drag.active && !(drag.item?.kind === "tab" && drag.item.paneIndex === paneIndex)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="drop-overlay"
                class:file-drag={drag.item?.kind === "file"}
              >
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="drop-zone drop-left"
                  class:active={dropTarget?.paneIndex === paneIndex &&
                    dropTarget.zone === "left"}
                  onmouseenter={() => {
                    dropTarget = { paneIndex, zone: "left" };
                  }}
                  onmouseleave={() => {
                    if (
                      dropTarget?.paneIndex === paneIndex &&
                      dropTarget.zone === "left"
                    )
                      dropTarget = null;
                  }}
                >
                  <span class="drop-label">Split Left</span>
                </div>
                {#if drag.item?.kind === "tab"}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="drop-zone drop-center"
                    class:active={dropTarget?.paneIndex === paneIndex &&
                      dropTarget.zone === "center"}
                    onmouseenter={() => {
                      dropTarget = { paneIndex, zone: "center" };
                    }}
                    onmouseleave={() => {
                      if (
                        dropTarget?.paneIndex === paneIndex &&
                        dropTarget.zone === "center"
                      )
                        dropTarget = null;
                    }}
                  >
                    <span class="drop-label">Move Here</span>
                  </div>
                {/if}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="drop-zone drop-right"
                  class:active={dropTarget?.paneIndex === paneIndex &&
                    dropTarget.zone === "right"}
                  onmouseenter={() => {
                    dropTarget = { paneIndex, zone: "right" };
                  }}
                  onmouseleave={() => {
                    if (
                      dropTarget?.paneIndex === paneIndex &&
                      dropTarget.zone === "right"
                    )
                      dropTarget = null;
                  }}
                >
                  <span class="drop-label">Split Right</span>
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>

      {#if showHistory && activeTab && activeTab.type === "markdown"}
        <HistoryPanel
          filePath={activeTab.path}
          onclose={() => (showHistory = false)}
          onrestore={(content) => {
            const pi = activePaneIndex;
            const ti = panes[pi].activeTabIndex;
            if (ti >= 0) {
              panes[pi].tabs[ti] = { ...panes[pi].tabs[ti], content };
              panes[pi].externalContentVersion++;
            }
          }}
        />
      {/if}
    </div>

    <StatusBar
      onlogout={handleLogout}
      onsettings={() => (showSettings = true)}
      onsync={runQuietSync}
      onswitchvault={handleSwitchVault}
      onhistory={() => (showHistory = !showHistory)}
      historyActive={showHistory}
    />
  </div>

  {#if showSettings}
    <Settings onclose={() => (showSettings = false)} />
  {/if}

  {#if showQuickSwitcher}
    <QuickSwitcher
      onselect={handleFileSelect}
      onclose={() => (showQuickSwitcher = false)}
    />
  {/if}
{:else}
  <Login />
{/if}

{#if tabContextMenu}
  <ContextMenu
    x={tabContextMenu.x}
    y={tabContextMenu.y}
    items={getTabContextMenuItems(
      tabContextMenu.paneIndex,
      tabContextMenu.tabIndex,
    )}
    onclose={() => {
      tabContextMenu = null;
    }}
  />
{/if}

<Toaster />

<svelte:window
  onkeydown={(e) => {
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === "f") {
      e.preventDefault();
      sidebarFocusSearch = true;
    }
    if ((e.metaKey || e.ctrlKey) && e.key === "p") {
      e.preventDefault();
      showQuickSwitcher = !showQuickSwitcher;
    }
  }}
  onmousemove={(e) => {
    if (drag.active) drag.move(e.clientX, e.clientY);
  }}
  onmouseup={async () => {
    if (drag.active) {
      if (dropTarget) {
        await executeDrop(dropTarget);
      } else if (drag.item?.kind === "file" && !drag.item.isDir) {
        // No split zone hit — signal the editor under the cursor to insert at that position
        drag.requestInsertAtCoords(drag.item.path, drag.x, drag.y);
      }
      drag.end();
      dropTarget = null;
    }
  }}
/>

{#if drag.active}
  <div class="drag-ghost" style="left: {drag.x + 14}px; top: {drag.y - 12}px">
    {drag.item?.label}
  </div>
{/if}

<style>
  .app-layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .main-area {
    flex: 1;
    display: flex;
    overflow: hidden;
  }

  /* ═══════════════════════════════════════════════════
	   Panes
	   ═══════════════════════════════════════════════════ */
  .panes-container {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-width: 0;
  }

  .panes-container.resizing {
    user-select: none;
    cursor: col-resize;
  }

  .pane-divider {
    flex: 0 0 4px;
    background: var(--border);
    cursor: col-resize;
    transition: background 0.15s;
    z-index: 1;
  }

  .pane-divider:hover,
  .panes-container.resizing .pane-divider {
    background: var(--accent-link);
  }

  .pane {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-primary);
    min-width: 0;
    position: relative;
  }

  /* ═══════════════════════════════════════════════════
	   Drag ghost
	   ═══════════════════════════════════════════════════ */
  .drag-ghost {
    position: fixed;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    padding: 4px 10px;
    font-size: 0.8rem;
    color: var(--text-primary);
    pointer-events: none;
    z-index: 9999;
    white-space: nowrap;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    opacity: 0.9;
  }

  /* ═══════════════════════════════════════════════════
	   Drop overlay
	   ═══════════════════════════════════════════════════ */
  .drop-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    z-index: 100;
    pointer-events: none;
    background: color-mix(in srgb, var(--accent-link) 6%, transparent);
  }

  .drop-zone {
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.1s;
    pointer-events: auto;
  }

  .drop-left {
    flex: 30;
    border-right: 1px dashed
      color-mix(in srgb, var(--accent-link) 40%, transparent);
  }
  .drop-center {
    flex: 40;
  }
  .drop-right {
    flex: 30;
    border-left: 1px dashed
      color-mix(in srgb, var(--accent-link) 40%, transparent);
  }

  /* For file drags: thin edge strips only, transparent center for cursor insertion */
  .drop-overlay.file-drag {
    background: transparent;
    justify-content: space-between;
  }
  .drop-overlay.file-drag .drop-left {
    flex: 0 0 56px;
    border-right: 2px solid
      color-mix(in srgb, var(--accent-link) 30%, transparent);
  }
  .drop-overlay.file-drag .drop-right {
    flex: 0 0 56px;
    border-left: 2px solid
      color-mix(in srgb, var(--accent-link) 30%, transparent);
  }
  .drop-overlay.file-drag .drop-left.active,
  .drop-overlay.file-drag .drop-right.active {
    background: color-mix(in srgb, var(--accent-link) 18%, transparent);
    border-color: var(--accent-link);
  }

  .drop-zone.active {
    background: color-mix(in srgb, var(--accent-link) 22%, transparent);
  }

  .drop-label {
    font-size: 0.7rem;
    font-weight: 500;
    color: var(--accent-link);
    background: var(--bg-secondary);
    padding: 3px 8px;
    border-radius: 4px;
    border: 1px solid color-mix(in srgb, var(--accent-link) 50%, transparent);
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.1s;
  }

  .drop-zone.active .drop-label {
    opacity: 1;
  }

  /* ═══════════════════════════════════════════════════
	   Tab Bar
	   ═══════════════════════════════════════════════════ */
  .tab-bar {
    display: flex;
    align-items: center;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    height: 38px;
    min-height: 38px;
    overflow: hidden;
  }

  .tabs-scroll {
    display: flex;
    flex: 1;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }

  .tabs-scroll::-webkit-scrollbar {
    display: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    height: 38px;
    background: none;
    border-right: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 0.8rem;
    font-family: var(--font-sans);
    cursor: pointer;
    white-space: nowrap;
    transition:
      color 0.1s,
      background 0.1s;
    min-width: 36px;
    flex-shrink: 1;
    user-select: none;
  }

  .tab:hover {
    color: var(--text-secondary);
    background: var(--bg-hover);
  }

  .tab.active {
    color: var(--text-primary);
    background: var(--bg-primary);
    border-bottom: 2px solid var(--accent-link);
  }

  .tab-label {
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;
    flex-shrink: 1;
    min-width: 0;
  }

  .tab-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    background: none;
    border: none;
    border-radius: 4px;
    color: var(--text-muted);
    transition:
      color 0.1s,
      background 0.1s;
    flex-shrink: 0;
    cursor: pointer;
  }

  .tab-close:hover {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  .pane-controls {
    display: flex;
    align-items: center;
    padding: 0 6px;
    gap: 2px;
    flex-shrink: 0;
    border-left: 1px solid var(--border);
    height: 100%;
  }

  .pane-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: none;
    border: none;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    transition:
      color 0.1s,
      background 0.1s;
  }

  .pane-btn:hover {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  /* ═══════════════════════════════════════════════════
	   Breadcrumbs
	   ═══════════════════════════════════════════════════ */
  .breadcrumbs {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 16px;
    background: var(--bg-primary);
    border-bottom: 1px solid var(--border-subtle);
    color: var(--text-muted);
    font-size: 0.78rem;
    min-height: 28px;
    overflow-x: auto;
    white-space: nowrap;
  }

  .crumb {
    color: var(--text-muted);
  }

  .crumb.current {
    color: var(--text-secondary);
  }

  /* ═══════════════════════════════════════════════════
	   Editor Area
	   ═══════════════════════════════════════════════════ */
  .editor-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-primary);
    position: relative;
  }

  .cached-editor-slot {
    display: contents;
  }

  .cached-editor-slot.cached-hidden {
    display: none;
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 0.95rem;
  }
</style>
