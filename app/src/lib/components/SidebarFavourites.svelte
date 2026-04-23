<script lang="ts">
  import { files } from "$lib/stores/files.svelte";
  import { favourites } from "$lib/stores/favourites.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { Star, FileText } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";
  import { displayPath, displayName } from "$lib/utils/sidebar-ops";
  import type { MenuTarget } from "$lib/utils/sidebar-menu";

  interface Props {
    onfileselect: (path: string) => void;
    oncontextmenu: (target: MenuTarget, x: number, y: number) => void;
  }

  let { onfileselect, oncontextmenu }: Props = $props();
</script>

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
      onclick={() => onfileselect(favPath)}
      oncontextmenu={(event) => {
        event.preventDefault();
        oncontextmenu(
          { kind: "entry", entry: { path: favPath, name, is_dir: false, modified: 0 } },
          event.clientX,
          event.clientY,
        );
      }}
    >
      <Star size={14} class="favourite-star" />
      <div class="search-result-text">
        <span class="search-result-name">{displayName(name)}</span>
        {#if displayPath(favPath, vault.vaultPath)}
          <span class="search-result-path">{displayPath(favPath, vault.vaultPath)}</span>
        {/if}
      </div>
    </button>
  {/each}
</div>

<style>
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

  .panel-content {
    flex: 1;
    overflow-y: auto;
    padding: 0 var(--space-sm) var(--space-sm);
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
</style>
