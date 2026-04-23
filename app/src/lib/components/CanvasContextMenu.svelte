<script lang="ts">
  import * as m from "$lib/paraglide/messages.js";
  import type { Tool } from "$lib/canvas/types";
  import { colorPresets, sizePresets } from "$lib/canvas/types";

  interface Props {
    x: number;
    y: number;
    tool: Tool;
    penColor: string;
    currentSize: number;
    onSizeChange: (v: number) => void;
    onClearAll: () => void;
    onClose: () => void;
  }

  let {
    x,
    y,
    tool = $bindable(),
    penColor = $bindable(),
    currentSize,
    onSizeChange,
    onClearAll,
    onClose,
  }: Props = $props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="ctx-backdrop" onmousedown={onClose}></div>
<div class="ctx-menu" style:left={`${x}px`} style:top={`${y}px`} role="menu">
  <div class="ctx-section-label">{m.canvas_tool()}</div>
  <button class="ctx-item" class:active={tool === "pen"} onclick={() => { tool = "pen"; onClose(); }} role="menuitem">
    {m.canvas_pen()}
  </button>
  <button class="ctx-item" class:active={tool === "eraser"} onclick={() => { tool = "eraser"; onClose(); }} role="menuitem">
    {m.canvas_eraser()}
  </button>
  <button class="ctx-item" class:active={tool === "text"} onclick={() => { tool = "text"; onClose(); }} role="menuitem">
    {m.canvas_text()}
  </button>
  <div class="ctx-sep"></div>

  <div class="ctx-section-label">{m.canvas_color()}</div>
  <div class="ctx-colors">
    {#each colorPresets as c}
      <button
        class="color-swatch"
        class:active={penColor === c}
        style:background={c}
        onclick={() => { penColor = c; onClose(); }}
        aria-label={c}
      ></button>
    {/each}
  </div>
  <div class="ctx-sep"></div>

  <div class="ctx-section-label">{m.canvas_size()}</div>
  <div class="ctx-sizes">
    {#each sizePresets as s}
      <button
        class="ctx-size-btn"
        class:active={currentSize === s}
        onclick={() => { onSizeChange(s); onClose(); }}
      >
        {s}
      </button>
    {/each}
  </div>
  <div class="ctx-sep"></div>

  <button class="ctx-item danger" onclick={onClearAll} role="menuitem">
    {m.canvas_clear_all()}
  </button>
</div>

<style>
  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 99;
  }

  .ctx-menu {
    position: fixed;
    z-index: 100;
    min-width: 180px;
    padding: 6px;
    background: var(--glass-bg);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-sm);
    box-shadow: var(--glass-shadow);
  }

  .ctx-section-label {
    padding: 4px 8px 2px;
    font-size: 0.7rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .ctx-item {
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
    transition: background 0.1s, color 0.1s;
  }

  .ctx-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .ctx-item.active {
    color: var(--accent);
  }

  .ctx-item.danger {
    color: var(--danger);
  }

  .ctx-item.danger:hover {
    background: rgba(239, 68, 68, 0.1);
  }

  .ctx-sep {
    height: 1px;
    background: var(--border);
    margin: 4px 0;
  }

  .ctx-colors {
    display: flex;
    gap: 4px;
    padding: 4px 8px;
    flex-wrap: wrap;
    max-width: 160px;
  }

  .color-swatch {
    width: 18px;
    height: 18px;
    min-width: 18px;
    min-height: 18px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    transition: border-color 0.1s, transform 0.1s;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.2);
    flex-shrink: 0;
    padding: 0;
    box-sizing: content-box;
  }

  .color-swatch:hover {
    transform: scale(1.2);
  }

  .color-swatch.active {
    border-color: var(--accent);
    transform: scale(1.15);
  }

  .ctx-sizes {
    display: flex;
    gap: 3px;
    padding: 4px 8px;
    flex-wrap: wrap;
  }

  .ctx-size-btn {
    width: 28px;
    height: 28px;
    min-width: 28px;
    min-height: 28px;
    padding: 0;
    border-radius: var(--radius-xs);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-secondary);
    font-size: 0.75rem;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
  }

  .ctx-size-btn:hover {
    background: var(--bg-hover);
  }

  .ctx-size-btn.active {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--bg-tertiary);
  }
</style>
