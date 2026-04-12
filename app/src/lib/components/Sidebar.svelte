<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import ContextMenu, { type ContextMenuItem } from "./ContextMenu.svelte";
  import FileTree from "./FileTree.svelte";
  import {
    writeFileBytes,
    fileExists,
    revealInFileManager,
    searchFiles,
    searchFileContents,
    replaceInFile,
    copyFile,
    copyDirectory,
    renameEntry,
    type FsEntry,
    type TreeEntry,
    type ContentMatch,
    type TagInfo,
  } from "$lib/fs/bridge";
  import { files } from "$lib/stores/files.svelte";
  import { favourites } from "$lib/stores/favourites.svelte";
  import { editor } from "$lib/stores/editor.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { clipboard } from "$lib/stores/clipboard.svelte";
  import { tags as tagsStore } from "$lib/stores/tags.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { IconButton } from "$lib/ui";
  import {
    FilePlus,
    FolderPlus,
    Files,
    Search,
    FileText,
    PanelLeftClose,
    PanelLeft,
    ArrowDownAZ,
    ArrowDownWideNarrow,
    ChevronsDownUp,
    Star,
    Replace,
    ReplaceAll,
    CaseSensitive,
    ChevronDown,
    ChevronRight,
    PenLine,
    Network,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";
  import { validateName } from "$lib/utils/filename";

  type SidebarView = "files" | "search" | "favourites";

  interface Props {
    onfileselect: (path: string) => void;
    onrenameentry: (from: string, to: string, isDir: boolean) => Promise<void>;
    ondeleteentry: (path: string, isDir: boolean) => Promise<void>;
    onopengraph: () => void;
    panelOpen: boolean;
    ontoggle: () => void;
    focusSearch?: boolean;
  }

  type MenuTarget =
    | { kind: "root"; path: string }
    | { kind: "entry"; entry: TreeEntry | FsEntry };

  let {
    onfileselect,
    onrenameentry,
    ondeleteentry,
    onopengraph,
    panelOpen,
    ontoggle,
    focusSearch = $bindable(false),
  }: Props = $props();
  let activeView = $state<SidebarView>("files");
  let menuTarget = $state<MenuTarget | null>(null);
  let menuX = $state(0);
  let menuY = $state(0);
  let sidebarPanelEl = $state<HTMLElement | null>(null);
  let unlistenDragDrop: (() => void) | null = null;

  // Resize state
  const PANEL_MIN = 180;
  const PANEL_MAX = 480;
  let panelWidth = $state(280);
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

  // Search state
  let searchQuery = $state("");
  let searchResults = $state<FsEntry[]>([]);
  let contentResults = $state<ContentMatch[]>([]);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let searchGeneration = 0;
  let searching = $state(false);
  let searchInput = $state<HTMLInputElement | null>(null);
  let replaceQuery = $state("");
  let showReplace = $state(false);
  let caseSensitive = $state(false);
  let collapsedFiles = $state<Set<string>>(new Set());

  // Tags state (used in search view with # prefix)
  let allTags = $derived(tagsStore.items);
  let tagsLoading = $derived(tagsStore.loading);
  let selectedTag = $state<string | null>(null);
  let tagSearchQuery = $state("");

  let filteredTags = $derived.by(() => {
    const q = tagSearchQuery.trim().toLowerCase();
    if (!q) return allTags;
    return allTags.filter((t) => t.tag.includes(q));
  });

  let selectedTagFiles = $derived.by(() => {
    if (!selectedTag) return [];
    return allTags.find((t) => t.tag === selectedTag)?.files ?? [];
  });

  let isTagMode = $derived(searchQuery.trimStart().startsWith("#"));

  async function loadTags() {
    if (!vault.vaultPath) return;
    await tagsStore.load(vault.vaultPath);
  }

  $effect(() => {
    if (activeView === "search" && vault.vaultPath && isTagMode) {
      loadTags();
    }
  });

  // Group content results by file
  let groupedResults = $derived.by(() => {
    const groups = new Map<string, { name: string; matches: ContentMatch[] }>();
    for (const match of contentResults) {
      if (!groups.has(match.path)) {
        groups.set(match.path, { name: match.name, matches: [] });
      }
      groups.get(match.path)!.matches.push(match);
    }
    return groups;
  });

  $effect(() => {
    if (activeView === "search" && panelOpen && searchInput) {
      searchInput.focus();
    }
  });

  $effect(() => {
    if (focusSearch) {
      activeView = "search";
      if (!panelOpen) ontoggle();
      focusSearch = false;
      // Focus will happen via the other effect
    }
  });

  function getBasePath(): string {
    return files.selectedFolder ?? vault.vaultPath ?? "";
  }

  let contextHint = $derived.by(() => {
    if (!files.selectedFolder || !vault.vaultPath) return "";
    const rel = files.selectedFolder.slice(vault.vaultPath.length + 1);
    return rel ? m.sidebar_in_folder({ folder: rel }) : "";
  });

  function handleSearchInput() {
    const q = searchQuery.trim();
    if (!q) {
      searchResults = [];
      contentResults = [];
      selectedTag = null;
      tagSearchQuery = "";
      return;
    }
    // Tag search mode: #tagname
    if (q.startsWith("#")) {
      searchResults = [];
      contentResults = [];
      selectedTag = null;
      tagSearchQuery = q.slice(1);
      return;
    }
    tagSearchQuery = "";
    selectedTag = null;
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(async () => {
      if (!vault.vaultPath) return;
      searching = true;
      const gen = ++searchGeneration;
      try {
        const [fileResults, contents] = await Promise.all([
          searchFiles(vault.vaultPath, q),
          searchFileContents(vault.vaultPath, q, caseSensitive),
        ]);
        // Discard results if a newer search has started
        if (gen !== searchGeneration) return;
        searchResults = fileResults;
        contentResults = contents;
      } catch (err) {
        if (gen !== searchGeneration) return;
        console.warn("Search failed:", err);
        searchResults = [];
        contentResults = [];
      } finally {
        if (gen === searchGeneration) searching = false;
      }
    }, 100);
  }

  function toggleFileCollapse(path: string) {
    const next = new Set(collapsedFiles);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
    }
    collapsedFiles = next;
  }

  async function handleReplaceInFile(path: string) {
    if (!replaceQuery && replaceQuery !== "") return;
    try {
      const count = await replaceInFile(
        path,
        searchQuery,
        replaceQuery,
        caseSensitive,
      );
      if (count > 0) {
        toast.success(`Replaced ${count} occurrence${count > 1 ? "s" : ""}`);
        handleSearchInput();
      }
    } catch (err) {
      toast.error(`Replace failed: ${String(err)}`);
    }
  }

  async function handleReplaceAll() {
    if (!vault.vaultPath) return;
    const paths = [...groupedResults.keys()];
    let total = 0;
    for (const path of paths) {
      try {
        const count = await replaceInFile(
          path,
          searchQuery,
          replaceQuery,
          caseSensitive,
        );
        total += count;
      } catch (err) {
        console.warn(`Replace in ${path} failed:`, err);
      }
    }
    if (total > 0) {
      toast.success(
        `Replaced ${total} occurrence${total > 1 ? "s" : ""} in ${paths.length} file${paths.length > 1 ? "s" : ""}`,
      );
      handleSearchInput();
    }
  }

  function toggleCaseSensitive() {
    caseSensitive = !caseSensitive;
    if (searchQuery.trim()) {
      handleSearchInput();
    }
  }

  function displayPath(fullPath: string): string {
    if (!vault.vaultPath) return fullPath;
    const rel = fullPath.slice(vault.vaultPath.length + 1);
    // Remove filename, show parent path
    const parts = rel.split("/");
    if (parts.length <= 1) return "";
    return parts.slice(0, -1).join("/");
  }

  function displayName(name: string): string {
    if (name.endsWith(".md")) return name.slice(0, -3);
    if (name.endsWith(".canvas")) return name.slice(0, -7);
    return name;
  }

  function highlightMatch(
    context: string,
    query: string,
    isCaseSensitive: boolean,
  ): string {
    if (!query) return escapeHtml(context);
    const flags = isCaseSensitive ? "g" : "gi";
    return escapeHtml(context).replace(
      new RegExp(escapeRegex(escapeHtml(query)), flags),
      '<strong class="search-highlight">$&</strong>',
    );
  }

  function escapeHtml(str: string): string {
    return str
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function escapeRegex(str: string): string {
    return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }

  function normalizeFileName(input: string): string | null {
    const name = input.trim();
    const error = validateName(name);
    if (error) {
      toast.error(error);
      return null;
    }
    return name.includes(".") ? name : `${name}.md`;
  }

  function normalizeDirName(input: string): string | null {
    const name = input.trim();
    const error = validateName(name);
    if (error) {
      toast.error(error);
      return null;
    }
    return name;
  }

  async function createUniqueFilePath(
    base: string,
    desiredName?: string,
  ): Promise<string | null> {
    let name = desiredName ? normalizeFileName(desiredName) : "Untitled.md";
    if (!name) return null;

    const extIndex = name.lastIndexOf(".");
    const stem = extIndex > 0 ? name.slice(0, extIndex) : name;
    const ext = extIndex > 0 ? name.slice(extIndex) : ".md";
    let candidate = `${base}/${name}`;
    let i = 1;
    while (await fileExists(candidate)) {
      candidate = `${base}/${stem} ${i}${ext}`;
      i++;
    }
    return candidate;
  }

  function closeContextMenu() {
    menuTarget = null;
  }

  function openRootContextMenu(event: MouseEvent) {
    if (!vault.vaultPath) return;
    event.preventDefault();
    menuX = event.clientX;
    menuY = event.clientY;
    menuTarget = { kind: "root", path: getBasePath() };
  }

  function openEntryContextMenu(entry: TreeEntry, event: MouseEvent) {
    event.preventDefault();
    menuX = event.clientX;
    menuY = event.clientY;
    menuTarget = { kind: "entry", entry };
  }

  async function handleNewFile(base = getBasePath(), desiredName?: string) {
    if (!vault.vaultPath) return;
    const path = await createUniqueFilePath(base, desiredName);
    if (!path) return;

    const encoder = new TextEncoder();
    await writeFileBytes(path, encoder.encode(""));

    if (base !== vault.vaultPath) {
      files.expandFolder(base);
    }

    await files.refresh(vault.vaultPath);
    editor.markLocalChange();
    onfileselect(path);
    activeView = "files";
  }

  async function handleNewCanvas(base = getBasePath()) {
    if (!vault.vaultPath) return;
    const path = await createUniqueFilePath(base, "Untitled.canvas");
    if (!path) return;

    const encoder = new TextEncoder();
    await writeFileBytes(path, encoder.encode(""));

    if (base !== vault.vaultPath) {
      files.expandFolder(base);
    }

    await files.refresh(vault.vaultPath);
    editor.markLocalChange();
    onfileselect(path);
    activeView = "files";
  }

  function handleStartNewFolder(base = getBasePath()) {
    // Expand the parent so the inline input is visible
    if (base !== vault.vaultPath) {
      files.expandFolder(base);
    }
    files.startNewFolder(base);
  }

  async function handleInlineRename(entry: TreeEntry, newName: string) {
    const sanitized = entry.is_dir
      ? normalizeDirName(newName)
      : normalizeFileName(newName);
    if (!sanitized || sanitized === entry.name) return;

    const parent = entry.path.slice(0, entry.path.lastIndexOf("/"));
    const newPath = `${parent}/${sanitized}`;
    if (await fileExists(newPath)) {
      toast.error(`Path already exists: ${sanitized}`);
      return;
    }

    try {
      await onrenameentry(entry.path, newPath, entry.is_dir);
    } catch (err) {
      toast.error(`Rename failed: ${String(err)}`);
    }
  }

  async function handleDeleteRequest(entry: FsEntry) {
    const confirmed = window.confirm(`${m.sidebar_delete()} “${entry.name}”?`);
    if (!confirmed) return;
    try {
      await ondeleteentry(entry.path, entry.is_dir);
    } catch (err) {
      toast.error(`Delete failed: ${String(err)}`);
    }
  }

  async function handleOpenInFinder(path: string) {
    try {
      await revealInFileManager(path);
    } catch (err) {
      toast.error(`Open in Finder failed: ${String(err)}`);
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
      if (entry.is_dir) {
        await copyDirectory(entry.path, candidate);
      } else {
        await copyFile(entry.path, candidate);
      }
      await files.refresh(vault.vaultPath);
    } catch (err) {
      toast.error(`Duplicate failed: ${String(err)}`);
    }
  }

  function collapseAll() {
    files.collapseAll();
  }

  // ─── Move entry (drag-and-drop within sidebar) ─────────────────────────
  async function handleMoveEntry(fromPath: string, toDir: string, isDir: boolean) {
    if (!vault.vaultPath) return;
    const name = fromPath.split("/").pop() ?? "";
    let dest = `${toDir}/${name}`;
    if (await fileExists(dest)) {
      toast.error(`"${name}" already exists in the target folder`);
      return;
    }
    try {
      await onrenameentry(fromPath, dest, isDir);
    } catch (err) {
      toast.error(`Move failed: ${String(err)}`);
    }
  }

  // ─── Clipboard operations ─────────────────────────────────────────────
  function handleCopy(entry?: { path: string; is_dir: boolean }) {
    if (entry && files.selectedEntries.size <= 1) {
      clipboard.copy([entry.path], [entry.is_dir]);
    } else {
      const sel = files.getSelectedAsList();
      if (sel.length === 0) return;
      clipboard.copy(sel.map((s) => s.path), sel.map((s) => s.isDir));
    }
    toast.success("Copied to clipboard");
  }

  function handleCut(entry?: { path: string; is_dir: boolean }) {
    if (entry && files.selectedEntries.size <= 1) {
      clipboard.cut([entry.path], [entry.is_dir]);
    } else {
      const sel = files.getSelectedAsList();
      if (sel.length === 0) return;
      clipboard.cut(sel.map((s) => s.path), sel.map((s) => s.isDir));
    }
    toast.success("Cut to clipboard");
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

      // Generate unique name if destination exists
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
          if (isDir) {
            await copyDirectory(srcPath, dest);
          } else {
            await copyFile(srcPath, dest);
          }
        } else {
          // Cut = move
          await onrenameentry(srcPath, dest, isDir);
        }
      } catch (err) {
        toast.error(`Paste failed: ${String(err)}`);
      }
    }

    // Expand the target folder so pasted items are visible
    if (targetDir !== vault.vaultPath) {
      await files.expandFolder(targetDir);
    }
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
    // Only handle when files view is active and panel is open
    if (activeView !== "files" || !panelOpen) return;
    // Don't intercept when focus is in a text input or the editor
    const el = document.activeElement;
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return;
    if (el instanceof HTMLElement && el.isContentEditable) return;

    const mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    const sel = files.selectedEntry;

    if (e.key === "c" && sel) {
      e.preventDefault();
      e.stopPropagation();
      handleCopy();
    } else if (e.key === "x" && sel) {
      e.preventDefault();
      e.stopPropagation();
      handleCut();
    } else if (e.key === "a") {
      e.preventDefault();
      e.stopPropagation();
      files.selectAll();
    } else if (e.key === "v" && clipboard.hasItems) {
      e.preventDefault();
      e.stopPropagation();
      handlePaste(getPasteTarget());
    }
  }

  // ─── External file drop (from OS file manager) ────────────────────────
  async function handleExternalDrop(paths: string[], position: { x: number; y: number }) {
    if (!vault.vaultPath || !sidebarPanelEl) return;
    const rect = sidebarPanelEl.getBoundingClientRect();
    if (position.x < rect.left || position.x > rect.right ||
        position.y < rect.top || position.y > rect.bottom) return;

    const targetDir = getPasteTarget() || vault.vaultPath;

    for (const srcPath of paths) {
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
        // Check if source is a directory
        // We always copy from external (OS) drops
        await copyFile(srcPath, dest).catch(async () => {
          // If copyFile fails, it might be a directory
          await copyDirectory(srcPath, dest);
        });
      } catch (err) {
        toast.error(`Import failed for ${name}: ${String(err)}`);
      }
    }

    if (targetDir !== vault.vaultPath) {
      await files.expandFolder(targetDir);
    }
    await files.refresh(vault.vaultPath);
    editor.markLocalChange();
    toast.success(`Imported ${paths.length} item${paths.length > 1 ? "s" : ""}`);
  }

  onMount(() => {
    window.addEventListener("keydown", handleSidebarKeydown);
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          handleExternalDrop(event.payload.paths, event.payload.position);
        }
      })
      .then((unlisten) => {
        unlistenDragDrop = unlisten;
      });
  });

  onDestroy(() => {
    window.removeEventListener("keydown", handleSidebarKeydown);
    unlistenDragDrop?.();
  });

  let menuItems = $derived.by((): ContextMenuItem[] => {
    if (!menuTarget) return [];
    if (menuTarget.kind === "root") {
      const targetPath = menuTarget.path;
      return [
        {
          label: m.sidebar_new_file(),
          onclick: () => handleNewFile(targetPath),
        },
        {
          label: m.sidebar_new_canvas(),
          onclick: () => handleNewCanvas(targetPath),
        },
        {
          label: m.sidebar_new_folder(),
          onclick: () => handleStartNewFolder(targetPath),
        },
        ...(clipboard.hasItems ? [{
          label: m.sidebar_paste(),
          onclick: () => handlePaste(targetPath),
        }] : []),
        {
          label: m.sidebar_open_in_finder(),
          onclick: () => handleOpenInFinder(targetPath),
        },
      ];
    }

    const entry = menuTarget.entry;
    const items: ContextMenuItem[] = [];
    if (entry.is_dir) {
      items.push(
        {
          label: m.sidebar_new_file(),
          onclick: () => handleNewFile(entry.path),
        },
        {
          label: m.sidebar_new_canvas(),
          onclick: () => handleNewCanvas(entry.path),
        },
        {
          label: m.sidebar_new_folder(),
          onclick: () => handleStartNewFolder(entry.path),
        },
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
        {
          label: m.sidebar_copy(),
          onclick: () => handleCopy(entry),
        },
        {
          label: m.sidebar_cut(),
          onclick: () => handleCut(entry),
        },
      );
      if (clipboard.hasItems) {
        const pasteDir = entry.is_dir
          ? entry.path
          : entry.path.slice(0, entry.path.lastIndexOf("/"));
        items.push({
          label: m.sidebar_paste(),
          onclick: () => handlePaste(pasteDir),
        });
      }
      items.push(
        {
          label: m.sidebar_rename(),
          onclick: () => files.startRename(entry.path),
        },
        { label: m.sidebar_duplicate(), onclick: () => handleDuplicate(entry) },
        {
          label: m.sidebar_delete(),
          onclick: () => handleDeleteRequest(entry),
          destructive: true,
        },
        {
          label: m.sidebar_open_in_finder(),
          onclick: () => handleOpenInFinder(entry.path),
        },
      );
    return items;
  });
</script>

<div class="sidebar-root">
  <!-- Icon Rail -->
  <div class="icon-rail">
    <button
      class="rail-btn"
      class:active={activeView === "files" && panelOpen}
      onclick={() => {
        if (panelOpen && activeView === "files") {
          ontoggle();
        } else {
          activeView = "files";
          if (!panelOpen) ontoggle();
        }
      }}
      title="Explorer"
    >
      <Files size={20} />
    </button>
    <button
      class="rail-btn"
      class:active={activeView === "search" && panelOpen}
      onclick={() => {
        if (panelOpen && activeView === "search") {
          ontoggle();
        } else {
          activeView = "search";
          if (!panelOpen) ontoggle();
        }
      }}
      title="Search"
    >
      <Search size={20} />
    </button>
    <button
      class="rail-btn"
      class:active={activeView === "favourites" && panelOpen}
      onclick={() => {
        if (panelOpen && activeView === "favourites") {
          ontoggle();
        } else {
          activeView = "favourites";
          if (!panelOpen) ontoggle();
        }
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
      <button class="rail-btn" onclick={ontoggle} title="Close panel">
        <PanelLeftClose size={20} />
      </button>
    {:else}
      <button class="rail-btn" onclick={ontoggle} title="Open panel">
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
              icon={files.sortOrder === "name"
                ? ArrowDownAZ
                : ArrowDownWideNarrow}
              size="sm"
              onclick={() => files.toggleSortOrder()}
              title={files.sortOrder === "name"
                ? m.sidebar_sort_by_date()
                : m.sidebar_sort_by_name()}
            />
            <IconButton
              icon={ChevronsDownUp}
              size="sm"
              onclick={collapseAll}
              title={m.sidebar_collapse_all()}
            />
            <IconButton
              icon={FilePlus}
              size="sm"
              onclick={() => handleNewFile()}
              title={m.sidebar_new_file()}
            />
            <IconButton
              icon={PenLine}
              size="sm"
              onclick={() => handleNewCanvas()}
              title={m.sidebar_new_canvas()}
            />
            <IconButton
              icon={FolderPlus}
              size="sm"
              onclick={() => handleStartNewFolder()}
              title={m.sidebar_new_folder()}
            />
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
          />
        </div>
      {:else if activeView === "search"}
        <div class="panel-header">
          <span class="panel-title">Search</span>
          {#if !isTagMode}
          <div class="panel-actions">
            <IconButton
              icon={Replace}
              size="sm"
              onclick={() => (showReplace = !showReplace)}
              title="Toggle replace"
              active={showReplace}
            />
          </div>
          {/if}
        </div>

        <div class="search-box">
          <Search size={14} />
          <input
            class="search-input"
            bind:this={searchInput}
            bind:value={searchQuery}
            oninput={handleSearchInput}
            placeholder="Search in files... (# for tags)"
          />
          {#if !isTagMode}
          <button
            class="search-toggle-btn"
            class:active={caseSensitive}
            onclick={toggleCaseSensitive}
            title="Case sensitive"
          >
            <CaseSensitive size={14} />
          </button>
          {/if}
        </div>

        {#if showReplace && !isTagMode}
          <div class="search-box replace-box">
            <Replace size={14} />
            <input
              class="search-input"
              bind:value={replaceQuery}
              placeholder="Replace..."
            />
            <button
              class="search-toggle-btn"
              onclick={handleReplaceAll}
              title="Replace all in all files"
              disabled={contentResults.length === 0}
            >
              <ReplaceAll size={14} />
            </button>
          </div>
        {/if}

        <div class="panel-content">
          {#if isTagMode}
            {#if selectedTag}
              <!-- Files with selected tag -->
              <div class="tag-back-row">
                <button
                  class="tag-back-btn"
                  onclick={() => {
                    selectedTag = null;
                    searchQuery = "#";
                    tagSearchQuery = "";
                  }}
                >
                  <ChevronRight size={12} style="transform:rotate(180deg)" />
                  <span class="tag-chip tag-chip-active" style="font-size:0.75rem"
                    >#{selectedTag}</span
                  >
                </button>
                <span class="tag-file-count"
                  >{selectedTagFiles.length}
                  {m.tags_files({ count: selectedTagFiles.length })}</span
                >
              </div>
              {#if selectedTagFiles.length === 0}
                <div class="search-empty">{m.tags_no_files()}</div>
              {/if}
              {#each selectedTagFiles as filePath (filePath)}
                {@const name = filePath.split("/").pop() ?? filePath}
                <button
                  class="search-result"
                  class:active={files.activeFile === filePath}
                  onclick={() => {
                    onfileselect(filePath);
                  }}
                >
                  <FileText size={14} />
                  <div class="search-result-text">
                    <span class="search-result-name">{displayName(name)}</span>
                    {#if displayPath(filePath)}
                      <span class="search-result-path"
                        >{displayPath(filePath)}</span
                      >
                    {/if}
                  </div>
                </button>
              {/each}
            {:else if tagsLoading}
              <div class="search-empty">{m.tags_loading()}</div>
            {:else if filteredTags.length === 0}
              <div class="search-empty">{m.tags_empty()}</div>
            {:else}
              <div class="tag-cloud">
                {#each filteredTags as info (info.tag)}
                  <button
                    class="tag-chip"
                    onclick={() => {
                      selectedTag = info.tag;
                      searchQuery = `#${info.tag}`;
                    }}
                    title="#{info.tag} — {info.count} {m.tags_files({
                      count: info.count,
                    })}"
                  >
                    #{info.tag}
                    <span class="tag-count">{info.count}</span>
                  </button>
                {/each}
              </div>
            {/if}
          {:else}
          {#if searchQuery.trim() && contentResults.length === 0 && searchResults.length === 0 && !searching}
            <div class="search-empty">No results found</div>
          {/if}

          {#if contentResults.length > 0}
            <div class="search-summary">
              {contentResults.length} result{contentResults.length > 1
                ? "s"
                : ""} in {groupedResults.size} file{groupedResults.size > 1
                ? "s"
                : ""}
            </div>
          {/if}

          {#each [...groupedResults] as [filePath, group] (filePath)}
            <div class="search-file-group">
              <button
                class="search-file-header"
                onclick={() => toggleFileCollapse(filePath)}
              >
                {#if collapsedFiles.has(filePath)}
                  <ChevronRight size={12} />
                {:else}
                  <ChevronDown size={12} />
                {/if}
                <FileText size={14} />
                <span class="search-file-name">{displayName(group.name)}</span>
                <span class="search-file-count">{group.matches.length}</span>
                {#if showReplace}
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <span
                    class="search-file-replace"
                    onclick={(e) => {
                      e.stopPropagation();
                      handleReplaceInFile(filePath);
                    }}
                    title="Replace all in this file"
                  >
                    <Replace size={12} />
                  </span>
                {/if}
              </button>
              {#if !collapsedFiles.has(filePath)}
                {#each group.matches as match}
                  <button
                    class="search-match-item"
                    onclick={() => {
                      onfileselect(match.path);
                    }}
                  >
                    <span class="search-match-line">L{match.line}</span>
                    <span class="search-match-context"
                      >{@html highlightMatch(
                        match.context,
                        searchQuery,
                        caseSensitive,
                      )}</span
                    >
                  </button>
                {/each}
              {/if}
            </div>
          {/each}

          {#if searchResults.length > 0 && contentResults.length > 0}
            <div class="search-section-divider">File names</div>
          {/if}

          {#each searchResults as result (result.path)}
            <button
              class="search-result"
              class:active={files.activeFile === result.path}
              onclick={() => {
                onfileselect(result.path);
              }}
            >
              <FileText size={14} />
              <div class="search-result-text">
                <span class="search-result-name"
                  >{displayName(result.name)}</span
                >
                {#if displayPath(result.path)}
                  <span class="search-result-path"
                    >{displayPath(result.path)}</span
                  >
                {/if}
              </div>
            </button>
          {/each}
          {/if}
        </div>
      {:else if activeView === "favourites"}
        <div class="panel-header">
          <span class="panel-title">{m.sidebar_favourites()}</span>
        </div>

        <div class="panel-content">
          {#if favourites.paths.size === 0}
            <div class="search-empty">{m.sidebar_favourites()}: 0</div>
          {/if}

          {#each [...favourites.paths] as favPath (favPath)}
            {@const name = favPath.split("/").pop() ?? favPath}
            <button
              class="search-result"
              class:active={files.activeFile === favPath}
              onclick={() => {
                onfileselect(favPath);
              }}
              oncontextmenu={(event) => {
                event.preventDefault();
                menuX = event.clientX;
                menuY = event.clientY;
                menuTarget = {
                  kind: "entry",
                  entry: { path: favPath, name, is_dir: false, modified: 0 },
                };
              }}
            >
              <Star size={14} class="favourite-star" />
              <div class="search-result-text">
                <span class="search-result-name">{displayName(name)}</span>
                {#if displayPath(favPath)}
                  <span class="search-result-path">{displayPath(favPath)}</span>
                {/if}
              </div>
            </button>
          {/each}
        </div>
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
  /* ═══════════════════════════════════════════════════
	   Root: icon rail + panel side by side
	   ═══════════════════════════════════════════════════ */
  .sidebar-root {
    display: flex;
    height: 100%;
  }

  /* ═══════════════════════════════════════════════════
	   Icon Rail — narrow strip with view icons
	   ═══════════════════════════════════════════════════ */
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
    transition:
      color 0.12s,
      background 0.12s;
  }

  .rail-btn:hover {
    color: var(--text-secondary);
    background: var(--bg-hover);
  }

  .rail-btn.active {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  /* ═══════════════════════════════════════════════════
	   Panel — main sidebar content area
	   ═══════════════════════════════════════════════════ */
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

  /* ═══════════════════════════════════════════════════
	   Search View
	   ═══════════════════════════════════════════════════ */
  .search-box {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 6px 10px;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
    font-size: 0.9rem;
    flex-shrink: 0;
  }

  .search-box:focus-within {
    color: var(--text-secondary);
  }

  .search-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 0.9rem;
    font-family: var(--font-sans);
    padding: 0;
    caret-color: var(--accent);
  }

  .search-input::placeholder {
    color: var(--text-muted);
  }

  .search-empty {
    padding: 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .search-result {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    background: none;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    font-size: 0.9rem;
    cursor: pointer;
    text-align: left;
  }

  .search-result:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .search-result.active {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .search-result-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .search-result-name {
    font-size: 0.9rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .search-result-path {
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .search-result :global(.favourite-star) {
    flex-shrink: 0;
    color: var(--text-muted);
    fill: currentColor;
  }

  /* ═══════════════════════════════════════════════════
	   Content Search Results
	   ═══════════════════════════════════════════════════ */
  .replace-box {
    border-top: none;
    margin-top: 0;
  }

  .search-toggle-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-muted);
    cursor: pointer;
    margin-right: 2px;
    flex-shrink: 0;
    transition:
      color 0.12s,
      background 0.12s;
  }

  .search-toggle-btn:hover:not(:disabled) {
    color: var(--text-primary);
  }

  .search-toggle-btn.active {
    color: var(--accent-link);
    background: var(--bg-tertiary);
  }

  .search-toggle-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .search-summary {
    padding: 4px 10px;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .search-section-divider {
    padding: 8px 10px 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    border-top: 1px solid var(--border-subtle);
    margin-top: 4px;
  }

  .search-file-group {
    margin-bottom: 2px;
  }

  .search-file-header {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    padding: 4px 6px;
    background: none;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    font-size: 0.85rem;
    cursor: pointer;
    text-align: left;
  }

  .search-file-header:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .search-file-name {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
  }

  .search-file-count {
    font-size: 0.7rem;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    padding: 1px 5px;
    border-radius: 8px;
    flex-shrink: 0;
  }

  .search-file-replace {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: var(--radius-xs);
    color: var(--text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition:
      color 0.12s,
      background 0.12s;
  }

  .search-file-replace:hover {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  .search-match-item {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    width: 100%;
    padding: 3px 8px 3px 26px;
    background: none;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    text-align: left;
    line-height: 1.4;
  }

  .search-match-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .search-match-line {
    flex-shrink: 0;
    font-size: 0.7rem;
    color: var(--text-muted);
    font-family: var(--font-mono);
    min-width: 28px;
    padding-top: 1px;
  }

  .search-match-context {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .search-match-context :global(.search-highlight) {
    color: var(--text-primary);
    background: rgba(234, 179, 8, 0.3);
    border-radius: 2px;
    padding: 0 1px;
  }

  /* ═══════════════════════════════
	   Tags view
	   ═══════════════════════════════ */
  .tag-cloud {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 10px;
  }

  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 0.78rem;
    font-family: var(--font-mono);
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      border-color 0.12s,
      color 0.12s,
      background 0.12s;
  }

  .tag-chip:hover,
  .tag-chip-active {
    border-color: var(--accent, var(--text-muted));
    color: var(--text-primary);
    background: var(--bg-secondary);
  }

  .tag-count {
    font-size: 0.7rem;
    color: var(--text-muted);
    background: var(--bg-primary);
    border-radius: 999px;
    padding: 0 5px;
    font-family: var(--font-sans);
  }

  .tag-back-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tag-back-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    padding: 2px 0;
  }

  .tag-back-btn:hover {
    color: var(--text-primary);
  }

  .tag-file-count {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-family: var(--font-sans);
  }
</style>
