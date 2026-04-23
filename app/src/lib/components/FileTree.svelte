<script lang="ts">
  import { onMount } from "svelte";
  import type { TreeEntry } from "$lib/fs/bridge";
  import { createDirectory } from "$lib/fs/bridge";
  import { editor } from "$lib/stores/editor.svelte";
  import { files } from "$lib/stores/files.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { favourites } from "$lib/stores/favourites.svelte";
  import { drag } from "$lib/stores/drag.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import * as m from "$lib/paraglide/messages.js";
  import { validateName } from "$lib/utils/filename";
  import { resolveResource } from "@tauri-apps/api/path";
  import {
    isDescendantOrSelf,
    handleFolderDrop as doFolderDrop,
    handleRootDrop as doRootDrop,
    tryNativeDrag,
    startDragEntry,
  } from "$lib/utils/file-tree-drag";
  import {
    ChevronRight,
    Folder,
    FolderOpen,
    FileText,
    Star,
  } from "lucide-svelte";

  interface Props {
    activeFile: string | null;
    onfileselect: (path: string) => void;
    oncontextmenuentry: (entry: TreeEntry, event: MouseEvent) => void;
    onrename: (entry: TreeEntry, newName: string) => void;
    onmoveentry: (fromPath: string, toDir: string, isDir: boolean) => Promise<void>;
    ondeleteentry: (path: string, isDir: boolean) => Promise<void>;
  }

  let { activeFile, onfileselect, oncontextmenuentry, onrename, onmoveentry, ondeleteentry }: Props =
    $props();


  let dropTargetFolder = $state<string | null>(null);

  // Virtual scroll: only visible rows are in the DOM.
  const ROW_HEIGHT = 32;
  const OVERSCAN = 5;

  let viewport = $state<HTMLDivElement | undefined>();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  let rows = $derived(files.flatTree);

  // New-folder row inserted into virtual scroll flow
  let newFolderInsertIdx = $derived.by(() => {
    const parent = files.pendingNewFolder;
    if (!parent) return -1;
    const idx = rows.findIndex((r) => r.path === parent);
    if (idx === -1) return 0;
    const depth = rows[idx].depth;
    let at = idx + 1;
    while (at < rows.length && rows[at].depth > depth) at++;
    return at;
  });

  let newFolderDepth = $derived.by(() => {
    const parent = files.pendingNewFolder;
    if (!parent || !vault.vaultPath) return 0;
    if (parent === vault.vaultPath) return 0;
    const idx = rows.findIndex((r) => r.path === parent);
    return idx >= 0 ? rows[idx].depth + 1 : 0;
  });

  let virtualCount = $derived(rows.length + (newFolderInsertIdx >= 0 ? 1 : 0));
  let totalHeight = $derived(virtualCount * ROW_HEIGHT);

  let visibleStart = $derived(
    Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN),
  );
  let visibleEnd = $derived(
    Math.min(
      virtualCount,
      Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN,
    ),
  );
  let slabTop = $derived(visibleStart * ROW_HEIGHT);

  type VisibleItem =
    | { kind: "row"; row: (typeof rows)[0] }
    | { kind: "new-folder" };

  let visibleItems = $derived.by(() => {
    const items: VisibleItem[] = [];
    const insertIdx = newFolderInsertIdx;
    for (let vi = visibleStart; vi < visibleEnd; vi++) {
      if (vi === insertIdx) {
        items.push({ kind: "new-folder" });
      } else {
        const ri = insertIdx >= 0 && vi > insertIdx ? vi - 1 : vi;
        if (ri >= 0 && ri < rows.length) {
          items.push({ kind: "row", row: rows[ri] });
        }
      }
    }
    return items;
  });

  // Drag support
  let suppressNextClick = false;
  const nativeDragState = { started: false };

  let dragIconPath = "";
  onMount(async () => {
    try {
      dragIconPath = await resolveResource("icons/32x32.png");
    } catch {
      dragIconPath = "";
    }
  });

  $effect(() => {
    if (drag.active) {
      function onMove(e: MouseEvent) {
        tryNativeDrag(e.clientX, e.clientY, dragIconPath, ondeleteentry, nativeDragState);
      }
      window.addEventListener("mousemove", onMove);
      return () => { window.removeEventListener("mousemove", onMove); };
    }
  });

  function startDrag(e: MouseEvent, entry: TreeEntry) {
    startDragEntry(e, entry, () => { suppressNextClick = true; });
  }

  function handleFolderDrop(e: MouseEvent, folderPath: string) {
    doFolderDrop(e, folderPath, onmoveentry, (v) => { dropTargetFolder = v; });
  }

  function handleRootDrop(e: MouseEvent) {
    if (!vault.vaultPath) return;
    doRootDrop(e, vault.vaultPath, onmoveentry, (v) => { dropTargetFolder = v; });
  }

  // Inline editing
  function submitRename(entry: TreeEntry, input: HTMLInputElement) {
    const newName = input.value.trim();
    files.cancelRename();
    if (newName && newName !== entry.name) onrename(entry, newName);
  }

  async function confirmNewFolder(name: string) {
    const parent = files.pendingNewFolder;
    if (!name.trim() || !parent || !vault.vaultPath) {
      files.cancelNewFolder();
      return;
    }
    const trimmed = name.trim();
    const error = validateName(trimmed);
    if (error) {
      toast.error(error);
      files.cancelNewFolder();
      return;
    }
    const folderPath = `${parent}/${trimmed}`;
    await createDirectory(folderPath);
    files.cancelNewFolder();
    await files.expandFolder(folderPath);
    files.setSelectedFolder(folderPath);
    await files.refresh();
    editor.markLocalChange();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="tree-viewport"
  bind:this={viewport}
  bind:clientHeight={viewportHeight}
  onscroll={() => {
    if (viewport) scrollTop = viewport.scrollTop;
  }}
  onclick={(e) => {
    if (!(e.target as HTMLElement).closest('.tree-row') && vault.vaultPath) {
      files.clearSelection();
      files.setSelectedFolder(vault.vaultPath);
    }
  }}
  onmouseup={(e) => handleRootDrop(e)}
>
  <!-- Full-height spacer keeps the scrollbar proportionate -->
  <div class="tree-spacer" style="height: {totalHeight}px;">
    <!-- Only the rows in [visibleStart, visibleEnd) are in the DOM -->
    <div class="tree-slab" style="top: {slabTop}px;">
      {#each visibleItems as item (item.kind === "row" ? item.row.path : "__new_folder__")}
        {#if item.kind === "new-folder"}
          <div
            class="tree-row new-folder-row"
            style="padding-left: {newFolderDepth * 16 + 8}px;"
          >
            <Folder size={16} />
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="inline-input"
              autofocus
              placeholder={m.folder_name_placeholder()}
              onblur={(e) => confirmNewFolder(e.currentTarget.value)}
              onkeydown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  confirmNewFolder(e.currentTarget.value);
                }
                if (e.key === "Escape") files.cancelNewFolder();
              }}
            />
          </div>
        {:else}
          {@const row = item.row}
          {@const indent = row.depth * 16}
          {@const isExpanded = files.expandedFolders.has(row.path)}
          {@const isRenaming = files.renamingPath === row.path}

          {#if row.is_dir}
            {#if isRenaming}
              <div class="tree-row" style="padding-left: {indent + 8}px;">
                {#if isExpanded}<FolderOpen size={16} />{:else}<Folder
                    size={16}
                  />{/if}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  class="inline-input"
                  autofocus
                  value={row.name}
                  onblur={(e) => submitRename(row, e.currentTarget)}
                  onkeydown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      submitRename(row, e.currentTarget);
                    }
                    if (e.key === "Escape") files.cancelRename();
                  }}
                />
              </div>
            {:else}
              <button
                class="tree-row folder-row"
                class:selected={files.isSelected(row.path)}
                class:drop-target={dropTargetFolder === row.path}
                style="padding-left: {indent + 4}px;"
                onmousedown={(e) => startDrag(e, row)}
                onclick={(e) => {
                  if (suppressNextClick) {
                    suppressNextClick = false;
                    return;
                  }
                  if (e.metaKey || e.ctrlKey) {
                    files.selectToggle(row.path, true);
                  } else if (e.shiftKey) {
                    files.selectRange(row.path);
                  } else {
                    files.selectSingle(row.path, true);
                    files.setSelectedFolder(row.path);
                    files.toggleFolder(row.path);
                  }
                }}
                ondblclick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  files.startRename(row.path);
                }}
                oncontextmenu={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  if (!files.isSelected(row.path)) {
                    files.selectSingle(row.path, true);
                  }
                  oncontextmenuentry(row, e);
                }}
                onmouseenter={() => {
                  if (drag.active && drag.item?.kind === "file" && !isDescendantOrSelf(drag.item.path, row.path)) {
                    dropTargetFolder = row.path;
                  }
                }}
                onmouseleave={() => {
                  if (dropTargetFolder === row.path) dropTargetFolder = null;
                }}
                onmouseup={(e) => handleFolderDrop(e, row.path)}
              >
                <span class="chevron" class:open={isExpanded}
                  ><ChevronRight size={14} /></span
                >
                {#if isExpanded}<FolderOpen size={16} />{:else}<Folder
                    size={16}
                  />{/if}
                <span class="row-name">{row.name}</span>
              </button>
            {/if}
          {:else if isRenaming}
            <div class="tree-row" style="padding-left: {indent + 8}px;">
              <FileText size={16} />
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="inline-input"
                autofocus
                value={row.name}
                onblur={(e) => submitRename(row, e.currentTarget)}
                onkeydown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    submitRename(row, e.currentTarget);
                  }
                  if (e.key === "Escape") files.cancelRename();
                }}
                onfocus={(e) => {
                  const val = e.currentTarget.value;
                  const dot = val.lastIndexOf(".");
                  e.currentTarget.setSelectionRange(
                    0,
                    dot > 0 ? dot : val.length,
                  );
                }}
              />
            </div>
          {:else}
            <button
              class="tree-row file-row"
              class:active={activeFile === row.path}
              class:selected={files.isSelected(row.path)}
              style="padding-left: {indent + 8}px;"
              onmousedown={(e) => startDrag(e, row)}
              onclick={(e) => {
                if (suppressNextClick) {
                  suppressNextClick = false;
                  return;
                }
                if (e.metaKey || e.ctrlKey) {
                  files.selectToggle(row.path, false);
                } else if (e.shiftKey) {
                  files.selectRange(row.path);
                } else {
                  files.selectSingle(row.path, false);
                  onfileselect(row.path);
                }
              }}
              ondblclick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                files.startRename(row.path);
              }}
              oncontextmenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                if (!files.isSelected(row.path)) {
                  files.selectSingle(row.path, false);
                }
                oncontextmenuentry(row, e);
              }}
            >
              <FileText size={16} />
              <span class="row-name"
                >{row.name.replace(/\.(md|canvas)$/, "")}</span
              >
              {#if favourites.isFavourite(row.path)}
                <Star size={12} class="favourite-star" />
              {/if}
            </button>
          {/if}
        {/if}
      {/each}
    </div>
  </div>
</div>

<style>
  .tree-viewport {
    overflow-y: auto;
    overflow-x: hidden;
    height: 100%;
    position: relative;
  }

  .tree-spacer {
    position: relative;
    width: 100%;
  }

  .tree-slab {
    position: absolute;
    left: 0;
    right: 0;
  }

  /* Every row — folder button, file button, or inline edit — is exactly ROW_HEIGHT tall */
  .tree-row {
    height: 32px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding-right: 12px;
    overflow: hidden;
  }

  .folder-row {
    background: none;
    color: var(--text-secondary);
    font-size: 0.95rem;
    cursor: pointer;
    border-radius: var(--radius-xs);
    text-align: left;
  }

  .folder-row:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .folder-row.selected {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .folder-row.drop-target {
    background: var(--accent);
    color: var(--bg-primary);
    opacity: 0.85;
  }

  .file-row {
    background: none;
    color: var(--text-secondary);
    font-size: 0.95rem;
    cursor: pointer;
    border-radius: var(--radius-xs);
    text-align: left;
  }

  .file-row:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .file-row.active {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .file-row.selected {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .new-folder-row {
    color: var(--text-secondary);
  }

  .chevron {
    display: flex;
    align-items: center;
    transition: transform 0.15s;
    flex-shrink: 0;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .row-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .tree-row :global(.favourite-star) {
    margin-left: auto;
    flex-shrink: 0;
    color: var(--text-muted);
    fill: currentColor;
  }

  .tree-row :global(svg) {
    flex-shrink: 0;
  }

  .inline-input {
    flex: 1;
    background: var(--bg-tertiary);
    border: 1px solid var(--text-muted);
    border-radius: var(--radius-xs);
    color: var(--text-primary);
    font-size: 0.95rem;
    font-family: var(--font-sans);
    padding: 2px 8px;
    outline: none;
    min-width: 0;
    height: 24px;
    box-sizing: border-box;
  }

  .inline-input::placeholder {
    color: var(--text-muted);
  }
</style>
