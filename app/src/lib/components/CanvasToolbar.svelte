<script lang="ts">
  import {
    Pencil, Eraser, Square, Circle, Minus, ArrowUpRight,
    ZoomIn, ZoomOut, RotateCcw, Hand, Type,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";
  import type { Tool } from "$lib/canvas/types";
  import { colorPresets } from "$lib/canvas/types";

  interface Props {
    tool: Tool;
    penColor: string;
    currentSize: number;
    onSizeChange: (v: number) => void;
    zoom: number;
    onZoomIn: () => void;
    onZoomOut: () => void;
    onResetView: () => void;
  }

  let {
    tool = $bindable(),
    penColor = $bindable(),
    currentSize,
    onSizeChange,
    zoom,
    onZoomIn,
    onZoomOut,
    onResetView,
  }: Props = $props();
</script>

<div class="toolbar">
  <div class="toolbar-group">
    <button class="tool-btn" class:active={tool === "hand"} onclick={() => (tool = "hand")} title={m.canvas_hand()}>
      <Hand size={16} />
    </button>
    <button class="tool-btn" class:active={tool === "pen"} onclick={() => (tool = "pen")} title={m.canvas_pen()}>
      <Pencil size={16} />
    </button>
    <button class="tool-btn" class:active={tool === "eraser"} onclick={() => (tool = "eraser")} title={m.canvas_eraser()}>
      <Eraser size={16} />
    </button>
    <span class="toolbar-sep"></span>
    <button class="tool-btn" class:active={tool === "rect"} onclick={() => (tool = "rect")} title={m.canvas_rect()}>
      <Square size={16} />
    </button>
    <button class="tool-btn" class:active={tool === "ellipse"} onclick={() => (tool = "ellipse")} title={m.canvas_ellipse()}>
      <Circle size={16} />
    </button>
    <button class="tool-btn" class:active={tool === "line"} onclick={() => (tool = "line")} title={m.canvas_line()}>
      <Minus size={16} />
    </button>
    <button class="tool-btn" class:active={tool === "arrow"} onclick={() => (tool = "arrow")} title={m.canvas_arrow()}>
      <ArrowUpRight size={16} />
    </button>
    <button class="tool-btn" class:active={tool === "text"} onclick={() => (tool = "text")} title={m.canvas_text()}>
      <Type size={16} />
    </button>
  </div>

  <span class="toolbar-sep"></span>

  <div class="toolbar-group colors">
    {#each colorPresets as c}
      <button
        class="color-swatch"
        class:active={penColor === c}
        style:background={c}
        onclick={() => (penColor = c)}
        title={c}
      ></button>
    {/each}
  </div>

  <span class="toolbar-sep"></span>

  <div class="toolbar-group size-group">
    <label class="size-label" for="canvas-size-slider">{currentSize}px</label>
    <input
      id="canvas-size-slider"
      type="range"
      min={tool === "text" ? 8 : 1}
      max={tool === "eraser" ? 60 : tool === "text" ? 72 : 30}
      value={currentSize}
      oninput={(e) => onSizeChange(Number(e.currentTarget.value))}
      class="size-slider"
    />
  </div>

  <span class="toolbar-sep"></span>

  <div class="toolbar-group">
    <button class="tool-btn" onclick={onZoomIn} title={m.canvas_zoom_in()}>
      <ZoomIn size={14} />
    </button>
    <span class="zoom-label">{Math.round(zoom * 100)}%</span>
    <button class="tool-btn" onclick={onZoomOut} title={m.canvas_zoom_out()}>
      <ZoomOut size={14} />
    </button>
    <button class="tool-btn" onclick={onResetView} title={m.canvas_reset_view()}>
      <RotateCcw size={14} />
    </button>
  </div>
</div>

<style>
  .toolbar {
    position: absolute;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background: var(--glass-bg);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-sm);
    box-shadow: var(--glass-shadow);
    z-index: 10;
    user-select: none;
    max-width: calc(100% - 32px);
    flex-wrap: wrap;
    justify-content: center;
  }

  .toolbar-group {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .toolbar-sep {
    width: 1px;
    height: 20px;
    background: var(--border);
    margin: 0 4px;
    flex-shrink: 0;
  }

  .tool-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    min-width: 30px;
    min-height: 30px;
    padding: 0;
    border: none;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
    flex-shrink: 0;
  }

  .tool-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .tool-btn.active {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .colors {
    gap: 3px;
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

  .size-group {
    gap: 6px;
  }

  .size-label {
    font-size: 0.75rem;
    color: var(--text-muted);
    min-width: 32px;
    text-align: right;
    flex-shrink: 0;
  }

  .size-slider {
    width: 80px;
    min-width: 60px;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .zoom-label {
    font-size: 0.75rem;
    color: var(--text-muted);
    min-width: 36px;
    text-align: center;
    flex-shrink: 0;
  }
</style>
