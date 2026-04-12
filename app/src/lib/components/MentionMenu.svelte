<script lang="ts">
  import type { MentionItem } from "$lib/editor/mention-command";
  import { FileText } from "lucide-svelte";

  interface Props {
    items: MentionItem[];
    selectedIndex: number;
    onselect: (item: MentionItem) => void;
    onhover: (index: number) => void;
  }

  let { items, selectedIndex, onselect, onhover }: Props = $props();

  let listEl: HTMLDivElement;

  export function scrollToSelected() {
    if (!listEl) return;
    const el = listEl.children[selectedIndex] as HTMLElement | undefined;
    if (!el) return;
    const top = el.offsetTop;
    const bottom = top + el.offsetHeight;
    if (top < listEl.scrollTop) {
      listEl.scrollTop = top;
    } else if (bottom > listEl.scrollTop + listEl.clientHeight) {
      listEl.scrollTop = bottom - listEl.clientHeight;
    }
  }
</script>

<div class="mention-menu" bind:this={listEl}>
  {#if items.length === 0}
    <div class="mention-menu-empty">No matching documents</div>
  {:else}
    {#each items as item, index (item.path)}
      <button
        class="mention-menu-item"
        class:is-selected={index === selectedIndex}
        onmouseenter={() => onhover(index)}
        onclick={() => onselect(item)}
      >
        <span class="mention-menu-icon"><FileText size={16} /></span>
        <div class="mention-menu-text">
          <span class="mention-menu-title">{item.title}</span>
        </div>
      </button>
    {/each}
  {/if}
</div>

<style>
  .mention-menu {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    max-height: 280px;
    overflow-y: auto;
    padding: 4px;
    min-width: 220px;
  }

  .mention-menu-empty {
    padding: 8px 12px;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .mention-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
    font-size: 0.875rem;
  }

  .mention-menu-item:hover,
  .mention-menu-item.is-selected {
    background: var(--bg-hover);
  }

  .mention-menu-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    color: var(--text-muted);
  }

  .mention-menu-text {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .mention-menu-title {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
