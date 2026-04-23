<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { writeFileBytes, readFileBytes } from "$lib/fs/bridge";
  import * as m from "$lib/paraglide/messages.js";
  import { theme } from "$lib/stores/theme.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { editor } from "$lib/stores/editor.svelte";
  import type { Tool, Stroke, Shape, TextLabel } from "$lib/canvas/types";
  import { colorPresets, sizePresets } from "$lib/canvas/types";
  import { serialize, deserialize } from "$lib/canvas/serialization";
  import { render as renderCanvas, type RenderOptions } from "$lib/canvas/renderer";
  import { SnapCache } from "$lib/canvas/snapping";
  import CanvasToolbar from "./CanvasToolbar.svelte";
  import CanvasContextMenu from "./CanvasContextMenu.svelte";

  interface Props {
    filePath: string;
    initialData?: string;
    onsave?: (content: string) => void;
  }

  let { filePath, initialData = "", onsave }: Props = $props();

  let canvasEl: HTMLCanvasElement;
  let wrapperEl: HTMLDivElement;
  let ctx: CanvasRenderingContext2D;

  let camX = $state(0);
  let camY = $state(0);
  let zoom = $state(1);
  let isPanning = $state(false);
  let panStartX = 0;
  let panStartY = 0;
  let camStartX = 0;
  let camStartY = 0;

  let tool = $state<Tool>("pen");
  let penColor = $state("#ffffff");
  let penSize = $state(3);
  let eraserSize = $state(20);
  let textFontSize = $state(16);

  let ctxMenu = $state<{ x: number; y: number } | null>(null);

  let strokes: Stroke[] = [];
  let shapes: Shape[] = [];
  let textLabels: TextLabel[] = [];
  let currentStroke: Stroke | null = null;
  let currentShape: Shape | null = null;
  let isDrawing = false;

  let editingText = $state<{ x: number; y: number; localX: number; localY: number } | null>(null);
  let textInputValue = $state("");
  let textInputEl = $state<HTMLInputElement>();
  let blurCommitBlocked = false;

  let activeSnap = $state<{ x: number; y: number } | null>(null);
  let activeSnapStart = $state<{ x: number; y: number } | null>(null);
  const snapCache = new SnapCache();

  let cursorX = $state(0);
  let cursorY = $state(0);
  let showCursor = $state(false);

  const offscreenRef: { canvas: HTMLCanvasElement | null; ctx: CanvasRenderingContext2D | null } = {
    canvas: null,
    ctx: null,
  };

  function doRender() {
    if (!ctx || !canvasEl) return;
    const opts: RenderOptions = {
      ctx, canvasEl, camX, camY, zoom,
      isDark: theme.current === "dark",
      strokes, shapes, textLabels,
      currentStroke, currentShape,
      activeSnap, activeSnapStart,
    };
    renderCanvas(opts, offscreenRef);
  }

  function emitChange() {
    const data = serialize(strokes, shapes, textLabels, camX, camY, zoom);
    onsave?.(data);
    editor.markLocalChange();
    const encoder = new TextEncoder();
    writeFileBytes(filePath, encoder.encode(data)).catch((err) => {
      console.error("Canvas save failed:", err);
      toast.error(m.toast_canvas_save_failed());
    });
  }

  function screenToWorld(sx: number, sy: number): { x: number; y: number } {
    const rect = canvasEl.getBoundingClientRect();
    return {
      x: (sx - rect.left) / zoom + camX,
      y: (sy - rect.top) / zoom + camY,
    };
  }

  function worldToLocal(wx: number, wy: number): { x: number; y: number } {
    return { x: (wx - camX) * zoom, y: (wy - camY) * zoom };
  }

  function resizeCanvas() {
    if (!canvasEl || !wrapperEl) return;
    const dpr = window.devicePixelRatio || 1;
    const rect = wrapperEl.getBoundingClientRect();
    canvasEl.width = rect.width * dpr;
    canvasEl.height = rect.height * dpr;
    canvasEl.style.width = `${rect.width}px`;
    canvasEl.style.height = `${rect.height}px`;
    ctx = canvasEl.getContext("2d")!;
    doRender();
  }

  function commitText() {
    const et = editingText;
    editingText = null;
    const val = textInputValue.trim();
    textInputValue = "";
    if (et && val) {
      textLabels.push({ x: et.x, y: et.y, text: val, color: penColor, fontSize: textFontSize });
      snapCache.invalidate();
      doRender();
      emitChange();
    }
  }

  function handlePointerDown(e: PointerEvent) {
    if (ctxMenu) { ctxMenu = null; return; }
    if (e.button === 1) { e.preventDefault(); startPan(e); return; }
    if (e.button === 0 && (spaceHeld || tool === "hand")) { startPan(e); return; }
    if (e.button !== 0) return;

    const pos = screenToWorld(e.clientX, e.clientY);

    if (tool === "text") {
      e.preventDefault();
      if (editingText) { blurCommitBlocked = true; commitText(); }
      const local = worldToLocal(pos.x, pos.y);
      editingText = { x: pos.x, y: pos.y, localX: local.x, localY: local.y };
      textInputValue = "";
      blurCommitBlocked = true;
      tick().then(() => { textInputEl?.focus(); blurCommitBlocked = false; });
      return;
    }

    if (tool === "pen" || tool === "eraser") {
      isDrawing = true;
      currentStroke = {
        points: [pos],
        color: tool === "eraser" ? "#000" : penColor,
        size: tool === "eraser" ? eraserSize : penSize,
        tool,
      };
      canvasEl.setPointerCapture(e.pointerId);
    } else {
      isDrawing = true;
      let startX = pos.x, startY = pos.y;
      if (tool === "line" || tool === "arrow") {
        const snap = snapCache.findNearest(pos.x, pos.y, zoom, shapes, textLabels, ctx);
        if (snap) { startX = snap.x; startY = snap.y; activeSnapStart = snap; }
        else activeSnapStart = null;
      }
      currentShape = { kind: tool as Shape["kind"], x1: startX, y1: startY, x2: startX, y2: startY, color: penColor, size: penSize };
      canvasEl.setPointerCapture(e.pointerId);
    }
    doRender();
  }

  function handlePointerMove(e: PointerEvent) {
    cursorX = e.clientX;
    cursorY = e.clientY;
    if (isPanning) {
      camX = camStartX - (e.clientX - panStartX) / zoom;
      camY = camStartY - (e.clientY - panStartY) / zoom;
      doRender();
      return;
    }
    if (!isDrawing) return;
    const pos = screenToWorld(e.clientX, e.clientY);
    if (currentStroke) {
      currentStroke.points.push(pos);
      currentStroke = currentStroke;
    } else if (currentShape) {
      if (currentShape.kind === "line" || currentShape.kind === "arrow") {
        const snap = snapCache.findNearest(pos.x, pos.y, zoom, shapes, textLabels, ctx);
        if (snap) { currentShape.x2 = snap.x; currentShape.y2 = snap.y; activeSnap = snap; }
        else { currentShape.x2 = pos.x; currentShape.y2 = pos.y; activeSnap = null; }
      } else { currentShape.x2 = pos.x; currentShape.y2 = pos.y; }
      currentShape = currentShape;
    }
    doRender();
  }

  function handlePointerUp(_e: PointerEvent) {
    if (isPanning) { isPanning = false; wrapperEl.style.cursor = ""; emitChange(); return; }
    if (!isDrawing) return;
    isDrawing = false;
    if (currentStroke && currentStroke.points.length >= 2) strokes.push(currentStroke);
    currentStroke = null;
    if (currentShape) {
      const dx = Math.abs(currentShape.x2 - currentShape.x1);
      const dy = Math.abs(currentShape.y2 - currentShape.y1);
      if (dx > 2 || dy > 2) { shapes.push(currentShape); snapCache.invalidate(); }
    }
    currentShape = null;
    activeSnap = null;
    activeSnapStart = null;
    doRender();
    emitChange();
  }

  function startPan(e: PointerEvent | MouseEvent) {
    isPanning = true;
    panStartX = e.clientX; panStartY = e.clientY;
    camStartX = camX; camStartY = camY;
    wrapperEl.style.cursor = "grabbing";
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      const factor = e.deltaY > 0 ? 0.9 : 1.1;
      const newZoom = Math.min(5, Math.max(0.1, zoom * factor));
      const rect = canvasEl.getBoundingClientRect();
      const mx = (e.clientX - rect.left) / zoom + camX;
      const my = (e.clientY - rect.top) / zoom + camY;
      camX = mx - (e.clientX - rect.left) / newZoom;
      camY = my - (e.clientY - rect.top) / newZoom;
      zoom = newZoom;
    } else {
      camX += e.deltaX / zoom;
      camY += e.deltaY / zoom;
    }
    doRender();
  }

  let spaceHeld = false;
  function handleKeyDown(e: KeyboardEvent) {
    if (e.code === "Space" && !e.repeat) {
      spaceHeld = true;
      if (!isDrawing) wrapperEl.style.cursor = "grab";
    }
    if ((e.ctrlKey || e.metaKey) && e.key === "z" && !e.shiftKey) {
      e.preventDefault();
      if (strokes.length > 0) strokes.pop();
      else if (shapes.length > 0) { shapes.pop(); snapCache.invalidate(); }
      else if (textLabels.length > 0) { textLabels.pop(); snapCache.invalidate(); }
      doRender();
      emitChange();
    }
  }

  function handleKeyUp(e: KeyboardEvent) {
    if (e.code === "Space") { spaceHeld = false; if (!isPanning) wrapperEl.style.cursor = ""; }
  }

  function handleContextMenu(e: MouseEvent) { e.preventDefault(); ctxMenu = { x: e.clientX, y: e.clientY }; }
  function closeContextMenu() { ctxMenu = null; }

  function handleClearAll() {
    strokes = [];
    shapes = [];
    textLabels = [];
    doRender();
    emitChange();
    closeContextMenu();
  }

  function setZoom(newZoom: number) { zoom = newZoom; doRender(); }
  function resetView() { zoom = 1; camX = 0; camY = 0; doRender(); }

  let resizeObserver: ResizeObserver;

  onMount(async () => {
    try {
      const bytes = await readFileBytes(filePath);
      const fileData = new TextDecoder().decode(bytes);
      const parsed = deserialize(fileData || initialData);
      if (parsed) {
        strokes = parsed.strokes;
        shapes = parsed.shapes;
        textLabels = parsed.textLabels;
        camX = parsed.camX;
        camY = parsed.camY;
        zoom = parsed.zoom;
        snapCache.invalidate();
      }
    } catch {
      const parsed = deserialize(initialData);
      if (parsed) {
        strokes = parsed.strokes;
        shapes = parsed.shapes;
        textLabels = parsed.textLabels;
        camX = parsed.camX;
        camY = parsed.camY;
        zoom = parsed.zoom;
        snapCache.invalidate();
      }
    }
    resizeCanvas();
    resizeObserver = new ResizeObserver(() => resizeCanvas());
    resizeObserver.observe(wrapperEl);
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    if (offscreenRef.canvas) {
      offscreenRef.canvas.width = 0;
      offscreenRef.canvas.height = 0;
      offscreenRef.canvas = null;
      offscreenRef.ctx = null;
    }
  });

  let currentSize = $derived(
    tool === "eraser" ? eraserSize : tool === "text" ? textFontSize : penSize,
  );
</script>

<svelte:window onkeydown={handleKeyDown} onkeyup={handleKeyUp} />

<div
  class="canvas-wrapper"
  class:tool-hand={tool === "hand"}
  class:tool-text={tool === "text"}
  bind:this={wrapperEl}
  oncontextmenu={handleContextMenu}
  onpointerenter={() => (showCursor = true)}
  onpointerleave={() => (showCursor = false)}
  role="application"
  aria-label="Canvas editor"
>
  <canvas
    bind:this={canvasEl}
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
    onwheel={handleWheel}
  ></canvas>

  <CanvasToolbar
    bind:tool
    bind:penColor
    {currentSize}
    onSizeChange={(v) => {
      if (tool === "eraser") eraserSize = v;
      else if (tool === "text") textFontSize = v;
      else penSize = v;
    }}
    {zoom}
    onZoomIn={() => setZoom(Math.min(5, zoom * 1.2))}
    onZoomOut={() => setZoom(Math.max(0.1, zoom / 1.2))}
    onResetView={resetView}
  />

  {#if ctxMenu}
    <CanvasContextMenu
      x={ctxMenu.x}
      y={ctxMenu.y}
      bind:tool
      bind:penColor
      {currentSize}
      onSizeChange={(v) => {
        if (tool === "eraser") eraserSize = v;
        else if (tool === "text") textFontSize = v;
        else penSize = v;
      }}
      onClearAll={handleClearAll}
      onClose={closeContextMenu}
    />
  {/if}

  {#if editingText}
    <input
      class="text-input-overlay"
      bind:this={textInputEl}
      style:left={`${editingText.localX}px`}
      style:top={`${editingText.localY}px`}
      style:font-size={`${textFontSize * zoom}px`}
      style:color={penColor}
      bind:value={textInputValue}
      onkeydown={(e) => {
        if (e.key === "Enter") commitText();
        if (e.key === "Escape") { editingText = null; textInputValue = ""; }
        e.stopPropagation();
      }}
      onblur={() => {
        if (blurCommitBlocked) { blurCommitBlocked = false; return; }
        commitText();
      }}
    />
  {/if}

  {#if showCursor && !isPanning && !ctxMenu && tool !== "text"}
    <div
      class="custom-cursor"
      style:left={`${cursorX}px`}
      style:top={`${cursorY}px`}
      style:width={`${currentSize * zoom}px`}
      style:height={`${currentSize * zoom}px`}
      style:border-color={tool === "eraser" ? "rgba(255,255,255,0.5)" : penColor}
    ></div>
  {/if}
</div>

<style>
  .canvas-wrapper {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--bg-primary);
    cursor: crosshair;
  }

  .canvas-wrapper:global(.tool-hand) {
    cursor: grab;
  }

  canvas {
    display: block;
    position: absolute;
    top: 0;
    left: 0;
  }

  .canvas-wrapper:global(.tool-text) {
    cursor: text;
  }

  .text-input-overlay {
    position: absolute;
    z-index: 20;
    background: transparent;
    border: 1px dashed var(--accent);
    outline: none;
    font-family: Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    padding: 2px 4px;
    min-width: 100px;
    caret-color: var(--accent);
    border-radius: 2px;
  }

  .custom-cursor {
    position: fixed;
    pointer-events: none;
    border: 1.5px solid;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    z-index: 50;
    transition: width 0.1s, height 0.1s;
    min-width: 4px;
    min-height: 4px;
  }
</style>
