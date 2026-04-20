<script lang="ts">
  import type { EmojiItem } from "$lib/editor/emoji-command";

  interface Props {
    items: EmojiItem[];
    selectedIndex: number;
    onselect: (item: EmojiItem) => void;
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

<div class="emoji-menu" bind:this={listEl}>
  {#if items.length === 0}
    <div class="emoji-menu-empty">No results</div>
  {:else}
    {#each items as item, index (item.emoji + item.name)}
      <button
        class="emoji-menu-item"
        class:is-selected={index === selectedIndex}
        onmouseenter={() => onhover(index)}
        onclick={() => onselect(item)}
      >
        <span class="emoji-menu-icon">{item.emoji}</span>
        <span class="emoji-menu-name">{item.name}</span>
      </button>
    {/each}
  {/if}
</div>

<style>
  .emoji-menu {
    max-height: 240px;
    overflow-y: auto;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-lg);
    padding: 4px;
    min-width: 220px;
  }

  .emoji-menu-empty {
    padding: 8px 12px;
    color: var(--text-secondary);
    font-size: 0.85rem;
  }

  .emoji-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 4px 8px;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-primary);
    font-size: 0.85rem;
    cursor: pointer;
    text-align: left;
  }

  .emoji-menu-item:hover,
  .emoji-menu-item.is-selected {
    background: var(--bg-hover);
  }

  .emoji-menu-icon {
    font-size: 1.2rem;
    width: 24px;
    text-align: center;
  }

  .emoji-menu-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
    font-size: 0.8rem;
  }
</style>
