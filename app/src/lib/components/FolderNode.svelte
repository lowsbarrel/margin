<script lang="ts">
  import type { FsEntry } from "$lib/fs/bridge";
  import { listDirectory, createDirectory } from "$lib/fs/bridge";
  import { files } from "$lib/stores/files.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import FolderNode from "./FolderNode.svelte";
  import {
    ChevronRight,
    Folder,
    FolderOpen,
    FileText,
    Star,
  } from "lucide-svelte";
  import { favourites } from "$lib/stores/favourites.svelte";
  import { drag } from "$lib/stores/drag.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { validateName } from "$lib/utils/filename";

  let suppressNextClick = false;

  function startFileDrag(e: MouseEvent, path: string, name: string) {
    if (e.button !== 0) return;
    const startX = e.clientX,
      startY = e.clientY;
    let didDrag = false;
    const label = name.replace(/\.(md|canvas)$/, "");

    function onMove(ev: MouseEvent) {
      if (
        !didDrag &&
        (Math.abs(ev.clientX - startX) > 4 || Math.abs(ev.clientY - startY) > 4)
      ) {
        didDrag = true;
        drag.start({ kind: "file", path, label, isDir: false }, ev.clientX, ev.clientY);
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

  interface Props {
    entry: FsEntry;
    activeFile: string | null;
    onfileselect: (path: string) => void;
    oncontextmenuentry: (entry: FsEntry, event: MouseEvent) => void;
    onrename: (entry: FsEntry, newName: string) => void;
  }

  let { entry, activeFile, onfileselect, oncontextmenuentry, onrename }: Props =
    $props();
  let children = $state<FsEntry[]>([]);
  // refreshVersion removed — FolderNode is superseded by the flat-tree FileTree.
  let lastRefresh = $state(0);

  let sortedChildren = $derived.by(() => {
    if (files.sortOrder === "name") return children;
    return [...children].sort((a, b) => {
      if (a.is_dir && !b.is_dir) return -1;
      if (!a.is_dir && b.is_dir) return 1;
      if (a.is_dir && b.is_dir)
        return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
      return (b.modified ?? 0) - (a.modified ?? 0);
    });
  });

  let expanded = $derived(files.expandedFolders.has(entry.path));
  let isSelected = $derived(files.selectedFolder === entry.path);
  let isPendingParent = $derived(files.pendingNewFolder === entry.path);
  let isRenaming = $derived(files.renamingPath === entry.path);

  function submitRename(input: HTMLInputElement) {
    const newName = input.value.trim();
    files.cancelRename();
    if (newName && newName !== entry.name) {
      onrename(entry, newName);
    }
  }

  function handleRenamingChild(childEntry: FsEntry, input: HTMLInputElement) {
    const newName = input.value.trim();
    files.cancelRename();
    if (newName && newName !== childEntry.name) {
      onrename(childEntry, newName);
    }
  }

  $effect(() => {
    if (!expanded) return;
    if (children.length === 0 || lastRefresh === 0) {
      lastRefresh = 1;
      listDirectory(entry.path).then((c) => (children = c));
    }
  });

  async function toggle() {
    files.setSelectedFolder(entry.path);
    files.toggleFolder(entry.path);
    if (!expanded && children.length === 0) {
      children = await listDirectory(entry.path);
      lastRefresh = 1;
    }
  }

  async function confirmNewFolder(name: string) {
    if (!name.trim() || !vault.vaultPath) {
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
    const folderPath = `${entry.path}/${trimmed}`;
    await createDirectory(folderPath);
    files.cancelNewFolder();
    files.expandFolder(folderPath);
    files.setSelectedFolder(folderPath);
    await files.refresh(vault.vaultPath);
  }
</script>

{#if isRenaming}
  <div class="inline-rename">
    {#if expanded}
      <FolderOpen size={16} />
    {:else}
      <Folder size={16} />
    {/if}
    <!-- svelte-ignore a11y_autofocus -->
    <input
      class="inline-input"
      autofocus
      value={entry.name}
      onblur={(e) => submitRename(e.currentTarget)}
      onkeydown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          submitRename(e.currentTarget);
        }
        if (e.key === "Escape") files.cancelRename();
      }}
      onfocus={(e) => {
        /* select text without extension */
        e.currentTarget.select();
      }}
    />
  </div>
{:else}
  <button
    class="folder-btn"
    class:selected={isSelected}
    onclick={toggle}
    ondblclick={(e) => {
      e.preventDefault();
      e.stopPropagation();
      files.startRename(entry.path);
    }}
    oncontextmenu={(event) => {
      event.preventDefault();
      event.stopPropagation();
      oncontextmenuentry(entry, event);
    }}
  >
    <span class="chevron" class:open={expanded}>
      <ChevronRight size={14} />
    </span>
    {#if expanded}
      <FolderOpen size={16} />
    {:else}
      <Folder size={16} />
    {/if}
    <span class="folder-name">{entry.name}</span>
  </button>
{/if}

{#if expanded || isPendingParent}
  <ul class="subtree">
    {#if isPendingParent}
      <li>
        <div class="inline-new-folder">
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
      </li>
    {/if}
    {#each sortedChildren as child (child.path)}
      <li>
        {#if child.is_dir}
          <FolderNode
            entry={child}
            {activeFile}
            {onfileselect}
            {oncontextmenuentry}
            {onrename}
          />
        {:else if files.renamingPath === child.path}
          <div class="inline-rename">
            <FileText size={16} />
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="inline-input"
              autofocus
              value={child.name}
              onblur={(e) => handleRenamingChild(child, e.currentTarget)}
              onkeydown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleRenamingChild(child, e.currentTarget);
                }
                if (e.key === "Escape") files.cancelRename();
              }}
              onfocus={(e) => {
                /* select name without .md extension */
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
            class="tree-file"
            class:active={activeFile === child.path}
            onmousedown={(e) => startFileDrag(e, child.path, child.name)}
            onclick={() => {
              if (suppressNextClick) {
                suppressNextClick = false;
                return;
              }
              onfileselect(child.path);
            }}
            ondblclick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              files.startRename(child.path);
            }}
            oncontextmenu={(event) => {
              event.preventDefault();
              event.stopPropagation();
              oncontextmenuentry(child, event);
            }}
          >
            <FileText size={16} />
            <span class="file-name"
              >{child.name.replace(/\.(md|canvas)$/, "")}</span
            >
            {#if favourites.isFavourite(child.path)}
              <Star size={12} class="favourite-star" />
            {/if}
          </button>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .folder-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 12px;
    background: none;
    color: var(--text-secondary);
    font-size: 0.95rem;
    cursor: pointer;
    border-radius: var(--radius-xs);
    text-align: left;
  }

  .folder-btn :global(svg),
  .tree-file :global(svg),
  .inline-rename :global(svg),
  .inline-new-folder :global(svg) {
    flex-shrink: 0;
  }

  .folder-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .folder-btn.selected {
    background: var(--bg-tertiary);
    color: var(--text-primary);
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

  .folder-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .subtree {
    list-style: none;
    padding-left: 20px;
    margin: 0;
  }

  .tree-file {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 12px;
    background: none;
    color: var(--text-secondary);
    font-size: 0.95rem;
    cursor: pointer;
    border-radius: var(--radius-xs);
    text-align: left;
  }

  .tree-file :global(.favourite-star) {
    margin-left: auto;
    flex-shrink: 0;
    color: var(--text-muted);
    fill: currentColor;
  }

  .tree-file:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .tree-file.active {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .inline-new-folder {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    color: var(--text-secondary);
  }

  .inline-rename {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    color: var(--text-secondary);
  }

  .inline-input {
    flex: 1;
    background: var(--bg-tertiary);
    border: 1px solid var(--text-muted);
    border-radius: var(--radius-xs);
    color: var(--text-primary);
    font-size: 0.95rem;
    font-family: var(--font-sans);
    padding: 3px 8px;
    outline: none;
    min-width: 0;
  }

  .inline-input::placeholder {
    color: var(--text-muted);
  }
</style>
