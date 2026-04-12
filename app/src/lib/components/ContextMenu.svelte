<script lang="ts">
  import { onMount, tick, untrack } from "svelte";

  export interface ContextMenuItem {
    label: string;
    onclick: () => void | Promise<void>;
    destructive?: boolean;
    disabled?: boolean;
  }

  interface Props {
    x: number;
    y: number;
    items: ContextMenuItem[];
    onclose: () => void;
  }

  let { x, y, items, onclose }: Props = $props();
  let menuEl: HTMLDivElement;
  let left = $state(untrack(() => x));
  let top = $state(untrack(() => y));

  async function positionMenu() {
    await tick();
    if (!menuEl) return;
    const rect = menuEl.getBoundingClientRect();
    left = Math.max(8, Math.min(x, window.innerWidth - rect.width - 8));
    top = Math.max(8, Math.min(y, window.innerHeight - rect.height - 8));
  }

  $effect(() => {
    x;
    y;
    positionMenu();
  });

  onMount(() => {
    positionMenu();
    requestAnimationFrame(() => menuEl?.focus());
  });

  function handleDocumentMouseDown(event: MouseEvent) {
    if (!menuEl?.contains(event.target as Node)) {
      onclose();
    }
  }

  function handleDocumentContextMenu(event: MouseEvent) {
    if (!menuEl?.contains(event.target as Node)) {
      onclose();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onclose();
    }
  }

  async function runItem(item: ContextMenuItem) {
    if (item.disabled) return;
    onclose();
    await item.onclick();
  }
</script>

<svelte:document
  onmousedown={handleDocumentMouseDown}
  oncontextmenu={handleDocumentContextMenu}
  onkeydown={handleKeydown}
/>

<div
  class="context-menu"
  bind:this={menuEl}
  style:left={`${left}px`}
  style:top={`${top}px`}
  tabindex={-1}
  role="menu"
>
  {#each items as item}
    <button
      class="menu-item"
      class:destructive={item.destructive}
      disabled={item.disabled}
      onclick={() => runItem(item)}
      role="menuitem"
    >
      {item.label}
    </button>
  {/each}
</div>

<style>
  .context-menu {
    position: fixed;
    z-index: 200;
    min-width: 160px;
    padding: 4px;
    background: var(--glass-bg);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-sm);
    box-shadow: var(--glass-shadow);
    outline: none;
  }

  .menu-item {
    display: block;
    width: 100%;
    padding: 6px 10px;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    font-size: 0.85rem;
    text-align: left;
    cursor: pointer;
    transition:
      background 0.1s,
      color 0.1s;
  }

  .menu-item:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .menu-item.destructive {
    color: var(--danger);
  }

  .menu-item.destructive:hover:not(:disabled) {
    background: rgba(239, 68, 68, 0.1);
    color: var(--danger-hover);
  }

  .menu-item:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
