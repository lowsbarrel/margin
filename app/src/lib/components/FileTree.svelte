<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import type { TreeEntry } from "$lib/fs/bridge";
  import { createDirectory } from "$lib/fs/bridge";
  import { editor } from "$lib/stores/editor.svelte";
  import { files } from "$lib/stores/files.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { favourites } from "$lib/stores/favourites.svelte";
  import { drag } from "$lib/stores/drag.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { validateName } from "$lib/utils/filename";
  import { startDrag as startNativeDrag } from "@crabnebula/tauri-plugin-drag";
  import { resolveResource } from "@tauri-apps/api/path";
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
  }

  let { activeFile, onfileselect, oncontextmenuentry, onrename, onmoveentry }: Props =
    $props();

  // ─── Drop target tracking ─────────────────────────────────────────────────
  let dropTargetFolder = $state<string | null>(null);

  // ─── Virtual scroll ───────────────────────────────────────────────────────
  // Only DOM nodes for the visible rows are created. Total height is maintained
  // via a full-height spacer so the scrollbar is accurate. OVERSCAN renders a
  // few extra rows above/below the viewport so fast scrolls don't flash blank.

  const ROW_HEIGHT = 32; // px — must match .tree-row height in CSS
  const OVERSCAN = 5;

  let viewport = $state<HTMLDivElement | undefined>();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  let rows = $derived(files.flatTree);

  // ─── Pending new-folder row ───────────────────────────────────────────────
  // Inserted into the virtual scroll flow so it pushes rows down naturally.

  let newFolderInsertIdx = $derived.by(() => {
    const parent = files.pendingNewFolder;
    if (!parent) return -1;
    const idx = rows.findIndex((r) => r.path === parent);
    if (idx === -1) {
      // Parent is the vault root — insert at the very top
      return 0;
    }
    // Insert after all children of this directory
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

  // ─── Drag support ─────────────────────────────────────────────────────────
  let suppressNextClick = false;
  let nativeDragStarted = false;

  /** Trigger native OS drag when cursor goes outside window bounds. */
  function tryNativeDrag(clientX: number, clientY: number) {
    if (!drag.active || nativeDragStarted) return;
    const item = drag.item;
    if (!item || item.kind !== "file") return;

    const margin = 2; // px tolerance
    const outside =
      clientX <= margin ||
      clientY <= margin ||
      clientX >= window.innerWidth - margin ||
      clientY >= window.innerHeight - margin;
    if (!outside) return;

    nativeDragStarted = true;
    drag.end();

    // Collect all selected file paths for multi-drag to desktop
    const paths =
      files.selectedEntries.size > 1 && files.isSelected(item.path)
        ? files.getSelectedPaths()
        : [item.path];

    startNativeDrag({
      item: paths,
      icon: dragIconPath,
    })
      .catch(() => {
        // drag cancelled or failed — that's fine
      })
      .finally(() => {
        nativeDragStarted = false;
      });
  }

  /** Track mouse globally while drag is active to detect window-leave. */
  function handleGlobalMouseMove(e: MouseEvent) {
    if (drag.active) {
      tryNativeDrag(e.clientX, e.clientY);
    }
  }

  let dragIconPath = "";
  onMount(async () => {
    try {
      dragIconPath = await resolveResource("icons/32x32.png");
    } catch {
      dragIconPath = "";
    }
    window.addEventListener("mousemove", handleGlobalMouseMove);
  });

  onDestroy(() => {
    window.removeEventListener("mousemove", handleGlobalMouseMove);
  });

  function startDrag(e: MouseEvent, entry: TreeEntry) {
    if (e.button !== 0) return;
    const startX = e.clientX,
      startY = e.clientY;
    let didDrag = false;
    const label = entry.name.replace(/\.(md|canvas)$/, "");

    function onMove(ev: MouseEvent) {
      if (
        !didDrag &&
        (Math.abs(ev.clientX - startX) > 4 || Math.abs(ev.clientY - startY) > 4)
      ) {
        didDrag = true;
        drag.start(
          { kind: "file", path: entry.path, label, isDir: entry.is_dir },
          ev.clientX,
          ev.clientY,
        );
        window.removeEventListener("mousemove", onMove);
      }
    }
    function onUp() {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      if (didDrag) suppressNextClick = true;
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  /** Check if `target` is the same as or a descendant of `source` */
  function isDescendantOrSelf(source: string, target: string): boolean {
    return target === source || target.startsWith(source + "/");
  }

  /** Collect entries to move: all selected if the dragged item is part of the selection, otherwise just the dragged item. */
  function getDropEntries(): { path: string; isDir: boolean }[] {
    const item = drag.item;
    if (!item || item.kind !== "file") return [];
    if (files.selectedEntries.size > 1 && files.isSelected(item.path)) {
      return files.getSelectedAsList();
    }
    return [{ path: item.path, isDir: item.isDir }];
  }

  function handleFolderDrop(e: MouseEvent, folderPath: string) {
    if (!drag.active || !drag.item || drag.item.kind !== "file") return;
    e.stopPropagation();
    const entries = getDropEntries();
    // Validate: skip entries that are ancestors of the target or already in the target
    const valid = entries.filter((entry) => {
      if (isDescendantOrSelf(entry.path, folderPath)) return false;
      const parent = entry.path.slice(0, entry.path.lastIndexOf("/"));
      return parent !== folderPath;
    });
    drag.end();
    dropTargetFolder = null;
    for (const entry of valid) {
      onmoveentry(entry.path, folderPath, entry.isDir);
    }
  }

  function handleRootDrop(e: MouseEvent) {
    if (!drag.active || !drag.item || drag.item.kind !== "file" || !vault.vaultPath) return;
    const entries = getDropEntries();
    const rootPath = vault.vaultPath;
    const valid = entries.filter((entry) => {
      const parent = entry.path.slice(0, entry.path.lastIndexOf("/"));
      return parent !== rootPath;
    });
    drag.end();
    dropTargetFolder = null;
    for (const entry of valid) {
      onmoveentry(entry.path, rootPath, entry.isDir);
    }
  }

  // ─── Inline editing ───────────────────────────────────────────────────────

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
<div
  class="tree-viewport"
  bind:this={viewport}
  bind:clientHeight={viewportHeight}
  onscroll={() => {
    if (viewport) scrollTop = viewport.scrollTop;
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
              placeholder="Folder name"
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
