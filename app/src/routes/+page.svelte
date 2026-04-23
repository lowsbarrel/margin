<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { files } from "$lib/stores/files.svelte";
  import { favourites } from "$lib/stores/favourites.svelte";
  import { editor } from "$lib/stores/editor.svelte";
  import Login from "$lib/components/Login.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import PaneView from "$lib/components/PaneView.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import HistoryPanel from "$lib/components/HistoryPanel.svelte";
  import {
    readFileBytes,
    onFileChanged,
    watchVault,
    unwatchFile,
    onVaultFsChanged,
    hasUnsyncedChanges,
  } from "$lib/fs/bridge";
  import { loadSettings } from "$lib/settings/bridge";
  import { s3Configure } from "$lib/s3/bridge";
  import {
    startAutoSync,
    stopAutoSync,
    runQuietSync,
    setSyncCredentials,
    type ConflictStrategy,
  } from "$lib/sync/s3sync";
  import { Toaster } from "$lib/ui";
  import * as m from "$lib/paraglide/messages.js";
  import { drag } from "$lib/stores/drag.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import type { ContextMenuItem } from "$lib/components/ContextMenu.svelte";
  import QuickSwitcher from "$lib/components/QuickSwitcher.svelte";
  import { checkForAppUpdate } from "$lib/utils/updater";
  import {
    saveWorkspaceState,
    loadWorkspaceState,
    type WorkspaceState,
  } from "$lib/settings/workspace";
  import type { SidebarView } from "$lib/components/Sidebar.svelte";
  import { panes, fileTitle } from "$lib/stores/panes.svelte";
  import { handleRename, handleDelete, handleWikiLink, handleLogout } from "$lib/utils/page-actions";
  import { handleTabMouseDown, executeDrop, startDividerDrag } from "$lib/utils/tab-drag";

  // Local UI state

  let showSettings = $state(false);
  let showQuickSwitcher = $state(false);
  let showHistory = $state(false);
  let sidebarOpen = $state(true);
  let sidebarFocusSearch = $state(false);
  let sidebarActiveView = $state<SidebarView>("files");
  let sidebarWidth = $state(280);
  let attachmentFolder = $state<string | null>(null);
  let pendingScrollText = $state<string | null>(null);
  let lastSaveTime = 0;
  let unlistenFileChange: (() => void) | null = null;
  let unlistenVaultChange: (() => void) | null = null;
  let updateInterval: ReturnType<typeof setInterval> | null = null;
  let panesContainerEl = $state<HTMLElement | null>(null);
  let dividerResizing = $state(false);
  let dropTarget = $state<{
    paneIndex: number;
    zone: "left" | "center" | "right";
  } | null>(null);

  // Scroll helpers

  function scrollEditorToText(searchText: string) {
    const tiptap = editor.tiptap;
    if (!tiptap?.state?.doc) {
      pendingScrollText = searchText;
      return;
    }
    doScrollToText(tiptap, searchText);
  }

  function doScrollToText(tiptap: import("@tiptap/core").Editor, searchText: string) {
    const doc = tiptap.state.doc;
    let targetPos = -1;
    doc.descendants((node, pos) => {
      if (targetPos >= 0) return false;
      if (node.isText && node.text?.includes(searchText)) {
        targetPos = pos + node.text.indexOf(searchText);
        return false;
      }
    });
    if (targetPos < 0) return;
    tiptap.commands.setTextSelection(targetPos);
    try {
      const view = tiptap.view;
      const coords = view.coordsAtPos(targetPos);
      const scrollContainer = view.dom.closest(".editor-container");
      if (scrollContainer && coords) {
        const rect = scrollContainer.getBoundingClientRect();
        const relativeTop = coords.top - rect.top + scrollContainer.scrollTop;
        scrollContainer.scrollTo({
          top: relativeTop - rect.height / 2,
          behavior: "smooth",
        });
      }
    } catch {
      /* ignore scroll errors */
    }
  }

  // File select

  async function handleFileSelect(path: string, searchText?: string) {
    const opened = await panes.openFile(path);
    if (opened && searchText) scrollEditorToText(searchText);
  }

  // Tab context menu

  let tabContextMenu = $state<{
    x: number;
    y: number;
    paneIndex: number;
    tabIndex: number;
  } | null>(null);

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
    const pane = panes.list[paneIndex];
    return [
      { label: m.tab_close(), onclick: () => panes.closeTab(paneIndex, tabIndex) },
      {
        label: m.tab_close_others(),
        onclick: () => panes.closeOtherTabs(paneIndex, tabIndex),
        disabled: pane.tabs.length <= 1,
      },
      { label: m.tab_close_all(), onclick: () => panes.closeAllTabs(paneIndex) },
    ];
  }

  // Logout / switch vault

  function onLogout() {
    handleLogout(() => {
      doSaveWorkspaceState();
      if (workspaceSaveTimer) clearTimeout(workspaceSaveTimer);
      workspaceRestored = false;
    });
  }

  // Workspace state persistence

  let workspaceSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let workspaceRestored = false;

  function scheduleWorkspaceSave() {
    if (!vault.isUnlocked || !vault.vaultPath || !vault.encryptionKey) return;
    if (!workspaceRestored) return;
    if (workspaceSaveTimer) clearTimeout(workspaceSaveTimer);
    workspaceSaveTimer = setTimeout(() => {
      workspaceSaveTimer = null;
      doSaveWorkspaceState();
    }, 1000);
  }

  function doSaveWorkspaceState() {
    if (!vault.vaultPath || !vault.encryptionKey) return;
    const wsState: WorkspaceState = {
      panes: panes.list.map((p) => ({
        tabs: p.tabs.map((t) => ({ path: t.path, type: t.type })),
        active_tab_index: p.activeTabIndex,
      })),
      pane_flexes: [...panes.flexes],
      active_pane_index: panes.activePaneIndex,
      expanded_folders: [...files.expandedFolders],
      sidebar_open: sidebarOpen,
      sidebar_width: sidebarWidth,
      sidebar_view: sidebarActiveView,
      sort_order: files.sortOrder,
    };
    saveWorkspaceState(vault.vaultPath, vault.encryptionKey, wsState).catch(
      (err) => console.warn("Failed to save workspace state:", err),
    );
  }

  async function restoreWorkspaceState() {
    if (!vault.vaultPath || !vault.encryptionKey) return;
    try {
      const ws = await loadWorkspaceState(vault.vaultPath, vault.encryptionKey);
      if (!ws || ws.panes.length === 0) {
        workspaceRestored = true;
        return;
      }

      sidebarOpen = ws.sidebar_open;
      sidebarWidth = ws.sidebar_width;
      sidebarActiveView = (ws.sidebar_view as SidebarView) || "files";

      if (ws.sort_order && ws.sort_order !== files.sortOrder) {
        files.setSortOrder(ws.sort_order as "name" | "date");
      }

      for (const folder of ws.expanded_folders) {
        files.expandedFolders.add(folder);
      }
      await files.refresh(vault.vaultPath);

      await panes.restoreFromWorkspace(
        ws.panes,
        ws.pane_flexes,
        ws.active_pane_index,
      );
    } catch (err) {
      console.warn("Failed to restore workspace state:", err);
    }
    workspaceRestored = true;
  }

  // Effects

  $effect(() => {
    if (!editor.dirty) lastSaveTime = Date.now();
  });

  $effect(() => {
    const tiptap = editor.tiptap;
    const text = pendingScrollText;
    if (tiptap?.state?.doc && text) {
      pendingScrollText = null;
      doScrollToText(tiptap, text);
    }
  });

  // Lifecycle

  onMount(() => {
    checkForAppUpdate();
    updateInterval = setInterval(checkForAppUpdate, 5 * 60 * 1000);

    onFileChanged(async () => {
      if (Date.now() - lastSaveTime < 2000) return;
      const tab = panes.activeTab;
      if (!tab) return;
      try {
        const bytes = await readFileBytes(tab.path);
        const newContent = new TextDecoder().decode(bytes);
        if (newContent !== tab.content) {
          const pi = panes.activePaneIndex;
          const ti = panes.list[pi].activeTabIndex;
          panes.list[pi].tabs[ti] = { ...panes.list[pi].tabs[ti], content: newContent };
          panes.list[pi].externalContentVersion++;
          editor.markLocalChange();
        }
      } catch (err) {
        console.warn("File may have been deleted:", err);
      }
    }).then((unlisten) => {
      unlistenFileChange = unlisten;
    });

    let vaultRefreshTimer: ReturnType<typeof setTimeout> | null = null;
    onVaultFsChanged(() => {
      editor.markLocalChange();
      if (vaultRefreshTimer) clearTimeout(vaultRefreshTimer);
      vaultRefreshTimer = setTimeout(() => {
        vaultRefreshTimer = null;
        files.refresh();
      }, 300);
    }).then((unlisten) => {
      unlistenVaultChange = unlisten;
    });
  });

  onDestroy(() => {
    if (workspaceSaveTimer) clearTimeout(workspaceSaveTimer);
    doSaveWorkspaceState();
    if (updateInterval) clearInterval(updateInterval);
    stopAutoSync();
    unwatchFile();
    unlistenFileChange?.();
    unlistenVaultChange?.();
  });

  // Vault setup

  $effect(() => {
    if (vault.isUnlocked && vault.vaultPath) {
      const currentVaultPath = vault.vaultPath;
      const currentKey = vault.encryptionKey;
      untrack(() => {
        files.refresh(currentVaultPath);
        favourites.load();
        restoreWorkspaceState();
        watchVault(currentVaultPath).catch((err) =>
          console.warn("Failed to start vault watcher:", err),
        );
        if (currentKey) {
          loadSettings(currentVaultPath, currentKey)
            .then((settings) => {
              if (vault.vaultPath !== currentVaultPath) return;
              if (settings?.s3) {
                s3Configure(settings.s3);
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
                if (currentKey) {
                  hasUnsyncedChanges(currentVaultPath, currentKey)
                    .then((pending) => {
                      if (vault.vaultPath !== currentVaultPath) return;
                      if (editor.syncStatus === "syncing") return;
                      editor.setSyncStatus(pending ? "idle" : "synced");
                    })
                    .catch(() => {
                      if (editor.syncStatus === "syncing") return;
                      editor.setSyncStatus("idle");
                    });
                }
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
      });
    }
  });

  // Auto-save workspace state when layout changes
  $effect(() => {
    const _panes = panes.list.map((p) => ({
      tabs: p.tabs.map((t) => t.path),
      activeTabIndex: p.activeTabIndex,
    }));
    const _flexes = panes.flexes;
    const _activePane = panes.activePaneIndex;
    const _sidebarOpen = sidebarOpen;
    const _sidebarWidth = sidebarWidth;
    const _sidebarView = sidebarActiveView;
    const _expanded = files.expandedFolders;
    const _sort = files.sortOrder;

    scheduleWorkspaceSave();
  });
</script>

{#if vault.isUnlocked}
  <div class="app-layout">
    <div class="main-area">
      <Sidebar
        onfileselect={handleFileSelect}
        onrenameentry={(from, to, isDir) => handleRename(from, to, isDir)}
        ondeleteentry={(path, isDir) => handleDelete(path, isDir)}
        onopengraph={() => panes.openGraph()}
        panelOpen={sidebarOpen}
        ontoggle={() => (sidebarOpen = !sidebarOpen)}
        bind:focusSearch={sidebarFocusSearch}
        bind:activeView={sidebarActiveView}
        bind:panelWidth={sidebarWidth}
      />

      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="panes-container"
        class:resizing={dividerResizing}
        bind:this={panesContainerEl}
      >
        {#each panes.list as pane, paneIndex (pane.id)}
          {#if paneIndex > 0}
            <div
              class="pane-divider"
              onmousedown={(e) => startDividerDrag(e, paneIndex - 1, panesContainerEl!, (v) => dividerResizing = v)}
            ></div>
          {/if}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="pane"
            class:pane-active={paneIndex === panes.activePaneIndex}
            style="flex: {panes.flexes[paneIndex]}"
            onmousedown={() => panes.focusPane(paneIndex)}
          >
            <PaneView
              {pane}
              {paneIndex}
              onfileselect={handleFileSelect}
              onrename={handleRename}
              onwikilink={handleWikiLink}
              ontabcontextmenu={handleTabContextMenu}
              {attachmentFolder}
              {dropTarget}
              ondropenter={(pi, zone) => { dropTarget = { paneIndex: pi, zone }; }}
              ondropleave={(pi, zone) => {
                if (dropTarget?.paneIndex === pi && dropTarget.zone === zone)
                  dropTarget = null;
              }}
            />
          </div>
        {/each}
      </div>

      {#if showHistory && panes.activeTab && panes.activeTab.type === "markdown"}
        <HistoryPanel
          filePath={panes.activeTab.path}
          onclose={() => (showHistory = false)}
          onrestore={(content) => {
            const pi = panes.activePaneIndex;
            const ti = panes.list[pi].activeTabIndex;
            if (ti >= 0) {
              panes.list[pi].tabs[ti] = { ...panes.list[pi].tabs[ti], content };
              panes.list[pi].externalContentVersion++;
            }
          }}
        />
      {/if}
    </div>

    <StatusBar
      onlogout={onLogout}
      onsettings={() => (showSettings = true)}
      onsync={runQuietSync}
      onswitchvault={onLogout}
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
</style>
