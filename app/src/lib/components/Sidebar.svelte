<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import ContextMenu, { type ContextMenuItem } from "./ContextMenu.svelte";
  import FileTree from "./FileTree.svelte";
  import SidebarSearch from "./SidebarSearch.svelte";
  import SidebarFavourites from "./SidebarFavourites.svelte";
  import {
    writeFileBytes,
    fileExists,
    revealInFileManager,
    copyFile,
    copyDirectory,
    renameEntry,
    type FsEntry,
    type TreeEntry,
  } from "$lib/fs/bridge";
  import { files } from "$lib/stores/files.svelte";
  import { favourites } from "$lib/stores/favourites.svelte";
  import { editor } from "$lib/stores/editor.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { clipboard } from "$lib/stores/clipboard.svelte";
  import { drag } from "$lib/stores/drag.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { IconButton } from "$lib/ui";
  import {
    FilePlus,
    FolderPlus,
    Files,
    Search,
    PanelLeftClose,
    PanelLeft,
    ArrowDownAZ,
    ArrowDownWideNarrow,
    ChevronsDownUp,
    Star,
    PenLine,
    Network,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";
  import {
    normalizeFileName,
    normalizeDirName,
    createUniqueFilePath,
  } from "$lib/utils/sidebar-ops";
  import {
    buildMenuItems,
    type MenuTarget,
  } from "$lib/utils/sidebar-menu";

  export type SidebarView = "files" | "search" | "favourites";

  interface Props {
    onfileselect: (path: string, searchText?: string) => void;
    onrenameentry: (from: string, to: string, isDir: boolean) => Promise<void>;
    ondeleteentry: (path: string, isDir: boolean) => Promise<void>;
    onopengraph: () => void;
    panelOpen: boolean;
    ontoggle: () => void;
    focusSearch?: boolean;
    activeView?: SidebarView;
    panelWidth?: number;
  }

  let {
    onfileselect,
    onrenameentry,
    ondeleteentry,
    onopengraph,
    panelOpen,
    ontoggle,
    focusSearch = $bindable(false),
    activeView = $bindable<SidebarView>("files"),
    panelWidth = $bindable(280),
  }: Props = $props();
  let menuTarget = $state<MenuTarget | null>(null);
  let menuX = $state(0);
  let menuY = $state(0);
  let sidebarPanelEl = $state<HTMLElement | null>(null);
  let unlistenDragDrop: (() => void) | null = null;

  const PANEL_MIN = 180;
  const PANEL_MAX = 480;
  let resizing = $state(false);

  function onResizeStart(e: MouseEvent) {
    e.preventDefault();
    resizing = true;
    const startX = e.clientX;
    const startW = panelWidth;

    function onMove(ev: MouseEvent) {
      const raw = startW + (ev.clientX - startX);
      panelWidth = Math.max(PANEL_MIN, Math.min(PANEL_MAX, raw));
    }

    function onUp() {
      resizing = false;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    }

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  $effect(() => {
    if (focusSearch) {
      activeView = "search";
      if (!panelOpen) ontoggle();
      focusSearch = false;
    }
  });

  function getBasePath(): string {
    return files.selectedFolder ?? vault.vaultPath ?? "";
  }

  async function ensureFolderExpanded(path: string) {
    if (!vault.vaultPath || path === vault.vaultPath || files.expandedFolders.has(path)) return;
    await files.expandFolder(path);
  }

  // Context menu
  function closeContextMenu() {
    menuTarget = null;
  }

  function openRootContextMenu(event: MouseEvent) {
    if (!vault.vaultPath) return;
    if ((event.target as HTMLElement).closest('.tree-row')) return;
    event.preventDefault();
    files.clearSelection();
    files.setSelectedFolder(vault.vaultPath);
    menuX = event.clientX;
    menuY = event.clientY;
    menuTarget = { kind: "root", path: vault.vaultPath };
  }

  function openEntryContextMenu(entry: TreeEntry, event: MouseEvent) {
    event.preventDefault();
    menuX = event.clientX;
    menuY = event.clientY;
    menuTarget = { kind: "entry", entry };
  }

  function openFavContextMenu(target: MenuTarget, x: number, y: number) {
    menuX = x;
    menuY = y;
    menuTarget = target;
  }

  // ─── File / folder creation ────────────────────────────────────────
  async function handleNewFile(base = getBasePath(), desiredName?: string) {
    if (!vault.vaultPath) return;
    const path = await createUniqueFilePath(base, desiredName);
    if (!path) return;

    const encoder = new TextEncoder();
    await writeFileBytes(path, encoder.encode(""));

    await ensureFolderExpanded(base);
    await files.refresh(vault.vaultPath);
    editor.markLocalChange();
    files.requestTreeReveal(path);
    onfileselect(path);
    activeView = "files";
  }

  async function handleNewCanvas(base = getBasePath()) {
    if (!vault.vaultPath) return;
    const path = await createUniqueFilePath(base, "Untitled.canvas");
    if (!path) return;

    const encoder = new TextEncoder();
    await writeFileBytes(path, encoder.encode(""));

    await ensureFolderExpanded(base);
    await files.refresh(vault.vaultPath);
    editor.markLocalChange();
    files.requestTreeReveal(path);
    onfileselect(path);
    activeView = "files";
  }

  async function handleStartNewFolder(base = getBasePath()) {
    if (!vault.vaultPath) return;
    await ensureFolderExpanded(base);
    files.startNewFolder(base);
    files.requestPendingNewFolderReveal(base);
    activeView = "files";
  }

  // Rename/delete/duplicate
  async function handleInlineRename(entry: TreeEntry, newName: string) {
    const sanitized = entry.is_dir
      ? normalizeDirName(newName)
      : normalizeFileName(newName);
    if (!sanitized || sanitized === entry.name) return;

    const parent = entry.path.slice(0, entry.path.lastIndexOf("/"));
    const newPath = `${parent}/${sanitized}`;
    if (await fileExists(newPath)) {
      toast.error(m.toast_path_exists({ name: sanitized }));
      return;
    }

    try {
      await onrenameentry(entry.path, newPath, entry.is_dir);
    } catch (err) {
      toast.error(m.toast_rename_failed({ error: String(err) }));
    }
  }

  async function handleDeleteRequest(entry: FsEntry) {
    const confirmed = window.confirm(`${m.sidebar_delete()} "${entry.name}"?`);
    if (!confirmed) return;
    try {
      await ondeleteentry(entry.path, entry.is_dir);
    } catch (err) {
      toast.error(m.toast_delete_failed({ error: String(err) }));
    }
  }

  async function handleOpenInFinder(path: string) {
    try {
      await revealInFileManager(path);
    } catch (err) {
      toast.error(m.toast_open_finder_failed({ error: String(err) }));
    }
  }

  async function handleDuplicate(entry: FsEntry) {
    if (!vault.vaultPath) return;
    const parent = entry.path.slice(0, entry.path.lastIndexOf("/"));
    const name = entry.name;
    const ext = name.includes(".") ? name.slice(name.lastIndexOf(".")) : "";
    const stem = ext ? name.slice(0, name.lastIndexOf(".")) : name;

    let candidate: string;
    let i = 1;
    do {
      const newName = `${stem} copy${i > 1 ? ` ${i}` : ""}${ext}`;
      candidate = `${parent}/${newName}`;
      i++;
    } while (await fileExists(candidate));

    try {
      if (entry.is_dir) await copyDirectory(entry.path, candidate);
      else await copyFile(entry.path, candidate);
      await files.refresh(vault.vaultPath);
    } catch (err) {
      toast.error(m.toast_duplicate_failed({ error: String(err) }));
    }
  }

  function collapseAll() {
    files.collapseAll();
  }

  // Move entry (sidebar drag-drop)
  async function handleMoveEntry(fromPath: string, toDir: string, isDir: boolean) {
    if (!vault.vaultPath) return;
    const name = fromPath.split("/").pop() ?? "";
    let dest = `${toDir}/${name}`;
    if (await fileExists(dest)) {
      toast.error(m.toast_exists_in_folder({ name }));
      return;
    }
    try {
      await onrenameentry(fromPath, dest, isDir);
    } catch (err) {
      toast.error(m.toast_move_failed({ error: String(err) }));
    }
  }

  // Clipboard
  function handleCopy(entry?: { path: string; is_dir: boolean }) {
    if (entry && files.selectedEntries.size <= 1) {
      clipboard.copy([entry.path], [entry.is_dir]);
    } else {
      const sel = files.getSelectedAsList();
      if (sel.length === 0) return;
      clipboard.copy(sel.map((s) => s.path), sel.map((s) => s.isDir));
    }
    toast.success(m.toast_copied());
  }

  function handleCut(entry?: { path: string; is_dir: boolean }) {
    if (entry && files.selectedEntries.size <= 1) {
      clipboard.cut([entry.path], [entry.is_dir]);
    } else {
      const sel = files.getSelectedAsList();
      if (sel.length === 0) return;
      clipboard.cut(sel.map((s) => s.path), sel.map((s) => s.isDir));
    }
    toast.success(m.toast_cut_clipboard());
  }

  async function handlePaste(targetDir: string) {
    if (!vault.vaultPath || !clipboard.hasItems) return;
    const data = clipboard.consume();
    if (!data) return;

    for (let i = 0; i < data.paths.length; i++) {
      const srcPath = data.paths[i];
      const isDir = data.isDirs[i];
      const name = srcPath.split("/").pop() ?? "";
      let dest = `${targetDir}/${name}`;

      if (await fileExists(dest)) {
        const ext = name.includes(".") ? name.slice(name.lastIndexOf(".")) : "";
        const stem = ext ? name.slice(0, name.lastIndexOf(".")) : name;
        let j = 1;
        do {
          const newName = `${stem} ${j}${ext}`;
          dest = `${targetDir}/${newName}`;
          j++;
        } while (await fileExists(dest));
      }

      try {
        if (data.operation === "copy") {
          if (isDir) await copyDirectory(srcPath, dest);
          else await copyFile(srcPath, dest);
        } else {
          await onrenameentry(srcPath, dest, isDir);
        }
      } catch (err) {
        toast.error(m.toast_paste_failed({ error: String(err) }));
      }
    }

    if (targetDir !== vault.vaultPath) await files.expandFolder(targetDir);
    await files.refresh(vault.vaultPath);
    editor.markLocalChange();
  }

  function getPasteTarget(): string {
    const sel = files.selectedEntry;
    if (sel?.isDir) return sel.path;
    if (files.selectedFolder) return files.selectedFolder;
    return vault.vaultPath ?? "";
  }

  function handleSidebarKeydown(e: KeyboardEvent) {
    if (activeView !== "files" || !panelOpen) return;
    const el = document.activeElement;
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return;
    if (el instanceof HTMLElement && el.isContentEditable) return;

    const mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    const sel = files.selectedEntry;

    if (e.key === "c" && sel) { e.preventDefault(); e.stopPropagation(); handleCopy(); }
    else if (e.key === "x" && sel) { e.preventDefault(); e.stopPropagation(); handleCut(); }
    else if (e.key === "a") { e.preventDefault(); e.stopPropagation(); files.selectAll(); }
    else if (e.key === "v" && clipboard.hasItems) { e.preventDefault(); e.stopPropagation(); handlePaste(getPasteTarget()); }
  }

  // External file drop (OS file manager)
  async function handleExternalDrop(paths: string[], position: { x: number; y: number }) {
    if (!vault.vaultPath || !sidebarPanelEl) return;
    // If this drop originated from our own native drag, flag it and skip import
    if (drag.nativeDragActive) {
      drag.markDroppedBackInApp();
      return;
    }
    const rect = sidebarPanelEl.getBoundingClientRect();
    if (position.x < rect.left || position.x > rect.right ||
        position.y < rect.top || position.y > rect.bottom) return;

    const targetDir = getPasteTarget() || vault.vaultPath;

    for (const srcPath of paths) {
      const name = srcPath.replace(/\\/g, "/").split("/").pop() ?? "";
      let dest = `${targetDir}/${name}`;

      if (await fileExists(dest)) {
        const ext = name.includes(".") ? name.slice(name.lastIndexOf(".")) : "";
        const stem = ext ? name.slice(0, name.lastIndexOf(".")) : name;
        let j = 1;
        do {
          const newName = `${stem} ${j}${ext}`;
          dest = `${targetDir}/${newName}`;
          j++;
        } while (await fileExists(dest));
      }

      try {
        await copyFile(srcPath, dest).catch(async () => {
          await copyDirectory(srcPath, dest);
        });
      } catch (err) {
        toast.error(m.toast_import_file_failed({ name, error: String(err) }));
      }
    }

    if (targetDir !== vault.vaultPath) await files.expandFolder(targetDir);
    await files.refresh(vault.vaultPath);
    editor.markLocalChange();
    toast.success(m.toast_imported_items({ count: String(paths.length) }));
  }

  onMount(() => {
    window.addEventListener("keydown", handleSidebarKeydown);
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          handleExternalDrop(event.payload.paths, event.payload.position);
        }
      })
      .then((unlisten) => { unlistenDragDrop = unlisten; });
  });

  onDestroy(() => {
    window.removeEventListener("keydown", handleSidebarKeydown);
    unlistenDragDrop?.();
  });

  let menuItems = $derived.by((): ContextMenuItem[] => {
    if (!menuTarget) return [];
    return buildMenuItems(menuTarget, {
      onNewFile: (base) => handleNewFile(base),
      onNewCanvas: (base) => handleNewCanvas(base),
      onNewFolder: (base) => handleStartNewFolder(base),
      onPaste: (dir) => handlePaste(dir),
      onOpenInFinder: (path) => handleOpenInFinder(path),
      onCopy: (entry) => handleCopy(entry),
      onCut: (entry) => handleCut(entry),
      onRename: (path) => files.startRename(path),
      onDuplicate: (entry) => handleDuplicate(entry),
      onDelete: (entry) => handleDeleteRequest(entry),
    });
  });
</script>

<div class="sidebar-root">
  <!-- Icon Rail -->
  <div class="icon-rail">
    <button
      class="rail-btn"
      class:active={activeView === "files" && panelOpen}
      onclick={() => {
        if (panelOpen && activeView === "files") ontoggle();
        else { activeView = "files"; if (!panelOpen) ontoggle(); }
      }}
      title={m.sidebar_explorer()}
    >
      <Files size={20} />
    </button>
    <button
      class="rail-btn"
      class:active={activeView === "search" && panelOpen}
      onclick={() => {
        if (panelOpen && activeView === "search") ontoggle();
        else { activeView = "search"; if (!panelOpen) ontoggle(); }
      }}
      title={m.sidebar_search_title()}
    >
      <Search size={20} />
    </button>
    <button
      class="rail-btn"
      class:active={activeView === "favourites" && panelOpen}
      onclick={() => {
        if (panelOpen && activeView === "favourites") ontoggle();
        else { activeView = "favourites"; if (!panelOpen) ontoggle(); }
      }}
      title={m.sidebar_favourites()}
    >
      <Star size={20} />
    </button>
    <button class="rail-btn" onclick={onopengraph} title={m.sidebar_graph()}>
      <Network size={20} />
    </button>

    <div class="rail-spacer"></div>

    {#if panelOpen}
      <button class="rail-btn" onclick={ontoggle} title={m.sidebar_close_panel()}>
        <PanelLeftClose size={20} />
      </button>
    {:else}
      <button class="rail-btn" onclick={ontoggle} title={m.sidebar_open_panel()}>
        <PanelLeft size={20} />
      </button>
    {/if}
  </div>

  <!-- Panel -->
  {#if panelOpen}
    <aside
      class="sidebar-panel"
      class:resizing
      style="width:{panelWidth}px;min-width:{panelWidth}px"
      bind:this={sidebarPanelEl}
    >
      {#if activeView === "files"}
        <div class="panel-header">
          <span class="panel-title">{m.sidebar_explorer()}</span>
          <div class="panel-actions">
            <IconButton
              icon={files.sortOrder === "name" ? ArrowDownAZ : ArrowDownWideNarrow}
              size="sm"
              onclick={() => files.toggleSortOrder()}
              title={files.sortOrder === "name" ? m.sidebar_sort_by_date() : m.sidebar_sort_by_name()}
            />
            <IconButton icon={ChevronsDownUp} size="sm" onclick={collapseAll} title={m.sidebar_collapse_all()} />
            <IconButton icon={FilePlus} size="sm" onclick={() => handleNewFile()} title={m.sidebar_new_file()} />
            <IconButton icon={PenLine} size="sm" onclick={() => handleNewCanvas()} title={m.sidebar_new_canvas()} />
            <IconButton icon={FolderPlus} size="sm" onclick={() => handleStartNewFolder()} title={m.sidebar_new_folder()} />
          </div>
        </div>

        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="panel-content" oncontextmenu={openRootContextMenu}>
          <FileTree
            activeFile={files.activeFile}
            {onfileselect}
            oncontextmenuentry={openEntryContextMenu}
            onrename={handleInlineRename}
            onmoveentry={handleMoveEntry}
            ondeleteentry={(path, isDir) => ondeleteentry(path, isDir)}
          />
        </div>
      {:else if activeView === "search"}
        <SidebarSearch
          {onfileselect}
          bind:focusSearch
          {panelOpen}
        />
      {:else if activeView === "favourites"}
        <SidebarFavourites
          onfileselect={(path) => onfileselect(path)}
          oncontextmenu={openFavContextMenu}
        />
      {/if}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="resize-handle" onmousedown={onResizeStart}></div>
    </aside>
  {/if}
</div>

{#if menuTarget && menuItems.length > 0}
  <ContextMenu
    x={menuX}
    y={menuY}
    items={menuItems}
    onclose={closeContextMenu}
  />
{/if}

<style>
  .sidebar-root {
    display: flex;
    height: 100%;
  }

  .icon-rail {
    width: 44px;
    min-width: 44px;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding-top: 12px;
    padding-bottom: 12px;
    gap: 4px;
    background: var(--bg-primary);
    border-right: 1px solid var(--border);
  }

  .rail-spacer {
    flex: 1;
  }

  .rail-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: 8px;
    color: var(--text-muted);
    cursor: pointer;
    transition: color 0.12s, background 0.12s;
  }

  .rail-btn:hover {
    color: var(--text-secondary);
    background: var(--bg-hover);
  }

  .rail-btn.active {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  .sidebar-panel {
    position: relative;
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
    overflow: hidden;
  }

  .sidebar-panel.resizing {
    user-select: none;
  }

  .resize-handle {
    position: absolute;
    top: 0;
    right: 0;
    width: 4px;
    height: 100%;
    cursor: col-resize;
    z-index: 10;
  }

  .resize-handle:hover,
  .sidebar-panel.resizing .resize-handle {
    background: var(--border);
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-lg) var(--space-lg) var(--space-sm);
  }

  .panel-title {
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-muted);
  }

  .panel-actions {
    display: flex;
    gap: 2px;
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    padding: 0 var(--space-sm) var(--space-sm);
  }
</style>
