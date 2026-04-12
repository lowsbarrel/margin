<script lang="ts">
  import type { SlashMenuItem } from "$lib/editor/slash-command";

  interface Props {
    items: SlashMenuItem[];
    selectedIndex: number;
    onselect: (item: SlashMenuItem) => void;
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

<div class="slash-menu" bind:this={listEl}>
  {#if items.length === 0}
    <div class="slash-menu-empty">No results</div>
  {:else}
    {#each items as item, index (item.title)}
      <button
        class="slash-menu-item"
        class:is-selected={index === selectedIndex}
        onmouseenter={() => onhover(index)}
        onclick={() => onselect(item)}
      >
        <span class="slash-menu-icon">{item.icon}</span>
        <div class="slash-menu-text">
          <span class="slash-menu-title">{item.title}</span>
          <span class="slash-menu-desc">{item.description}</span>
        </div>
      </button>
    {/each}
  {/if}
</div>
