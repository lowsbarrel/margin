<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { writeFileBytes, readFileBytes } from "$lib/fs/bridge";
  import {
    Pencil,
    Eraser,
    Square,
    Circle,
    Minus,
    ArrowUpRight,
    ZoomIn,
    ZoomOut,
    RotateCcw,
    Hand,
    Type,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";
  import { theme } from "$lib/stores/theme.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { editor } from "$lib/stores/editor.svelte";

  interface Props {
    filePath: string;
    initialData?: string;
    onsave?: (content: string) => void;
  }

  let { filePath, initialData = "", onsave }: Props = $props();

  // --- Canvas state ---
  let canvasEl: HTMLCanvasElement;
  let wrapperEl: HTMLDivElement;
  let ctx: CanvasRenderingContext2D;

  // Camera / pan
  let camX = $state(0);
  let camY = $state(0);
  let zoom = $state(1);
  let isPanning = $state(false);
  let panStartX = 0;
  let panStartY = 0;
  let camStartX = 0;
  let camStartY = 0;

  // Tool state
  type Tool =
    | "hand"
    | "pen"
    | "eraser"
    | "rect"
    | "ellipse"
    | "line"
    | "arrow"
    | "text";
  let tool = $state<Tool>("pen");
  let penColor = $state("#ffffff");
  let penSize = $state(3);
  let eraserSize = $state(20);

  // Context menu
  let ctxMenu = $state<{ x: number; y: number } | null>(null);

  // Drawing data
  interface Stroke {
    points: { x: number; y: number }[];
    color: string;
    size: number;
    tool: "pen" | "eraser";
  }

  interface Shape {
    kind: "rect" | "ellipse" | "line" | "arrow";
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    color: string;
    size: number;
  }

  let strokes: Stroke[] = [];
  let shapes: Shape[] = [];
  interface TextLabel {
    x: number;
    y: number;
    text: string;
    color: string;
    fontSize: number;
  }

  let textLabels: TextLabel[] = [];
  let currentStroke: Stroke | null = null;
  let currentShape: Shape | null = null;
  let isDrawing = false;

  // Text editing state
  let editingText = $state<{
    x: number;
    y: number;
    localX: number;
    localY: number;
  } | null>(null);
  let textInputValue = $state("");
  let textFontSize = $state(16);
  let textInputEl = $state<HTMLInputElement>();
  let blurCommitBlocked = false;

  // Snap state
  let activeSnap = $state<{ x: number; y: number } | null>(null);
  let activeSnapStart = $state<{ x: number; y: number } | null>(null);
  const SNAP_THRESHOLD = 12;

  // Cursor tracking for custom cursor
  let cursorX = $state(0);
  let cursorY = $state(0);
  let showCursor = $state(false);

  // Color presets
  const colorPresets = [
    "#ffffff",
    "#ef4444",
    "#f97316",
    "#eab308",
    "#22c55e",
    "#3b82f6",
    "#8b5cf6",
    "#ec4899",
    "#6b7280",
    "#000000",
  ];

  const sizePresets = [1, 2, 3, 5, 8, 12, 20];

  // ---------- Serialization ----------
  function serialize(): string {
    return JSON.stringify({ strokes, shapes, textLabels, camX, camY, zoom });
  }

  function deserialize(raw: string) {
    if (!raw) return;
    try {
      const data = JSON.parse(raw);
      strokes = data.strokes ?? [];
      shapes = data.shapes ?? [];
      textLabels = data.textLabels ?? [];
      camX = data.camX ?? 0;
      camY = data.camY ?? 0;
      zoom = data.zoom ?? 1;
      invalidateSnapPoints();
    } catch {
      console.warn("Canvas: could not parse saved drawing data");
    }
  }

  function emitChange() {
    const data = serialize();
    onsave?.(data);
    editor.markLocalChange();
    const encoder = new TextEncoder();
    writeFileBytes(filePath, encoder.encode(data)).catch((err) => {
      console.error("Canvas save failed:", err);
      toast.error("Failed to save canvas");
    });
  }

  // ---------- Coordinate helpers ----------
  function screenToWorld(sx: number, sy: number): { x: number; y: number } {
    const rect = canvasEl.getBoundingClientRect();
    return {
      x: (sx - rect.left) / zoom + camX,
      y: (sy - rect.top) / zoom + camY,
    };
  }

  function worldToLocal(wx: number, wy: number): { x: number; y: number } {
    return {
      x: (wx - camX) * zoom,
      y: (wy - camY) * zoom,
    };
  }

  // ---------- Snapping ----------
  const textWidthCache = new Map<string, number>();
  function measureTextWidth(text: string, fontSize: number): number {
    const key = `${fontSize}:${text}`;
    const cached = textWidthCache.get(key);
    if (cached !== undefined) return cached;
    if (!ctx) return text.length * fontSize * 0.6;
    ctx.save();
    ctx.font = `${fontSize}px Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`;
    const w = ctx.measureText(text).width;
    ctx.restore();
    textWidthCache.set(key, w);
    return w;
  }

  // Cached snap points — rebuilt only when shapes/textLabels change, not per mousemove.
  let snapPointsCache: { x: number; y: number }[] = [];
  let snapPointsDirty = true;

  function invalidateSnapPoints() {
    snapPointsDirty = true;
  }

  function buildSnapPoints(): { x: number; y: number }[] {
    const pts: { x: number; y: number }[] = [];
    for (const s of shapes) {
      if (s.kind === "rect") {
        const x1 = Math.min(s.x1, s.x2),
          y1 = Math.min(s.y1, s.y2);
        const x2 = Math.max(s.x1, s.x2),
          y2 = Math.max(s.y1, s.y2);
        const mx = (x1 + x2) / 2,
          my = (y1 + y2) / 2;
        pts.push(
          { x: x1, y: y1 },
          { x: x2, y: y1 },
          { x: x2, y: y2 },
          { x: x1, y: y2 },
        );
        pts.push(
          { x: mx, y: y1 },
          { x: x2, y: my },
          { x: mx, y: y2 },
          { x: x1, y: my },
        );
      } else if (s.kind === "ellipse") {
        const cx = (s.x1 + s.x2) / 2,
          cy = (s.y1 + s.y2) / 2;
        const rx = Math.abs(s.x2 - s.x1) / 2,
          ry = Math.abs(s.y2 - s.y1) / 2;
        pts.push(
          { x: cx, y: cy - ry },
          { x: cx + rx, y: cy },
          { x: cx, y: cy + ry },
          { x: cx - rx, y: cy },
        );
      } else if (s.kind === "line" || s.kind === "arrow") {
        pts.push({ x: s.x1, y: s.y1 }, { x: s.x2, y: s.y2 });
      }
    }
    for (const t of textLabels) {
      const w = measureTextWidth(t.text, t.fontSize);
      const h = t.fontSize * 1.2;
      pts.push(
        { x: t.x, y: t.y },
        { x: t.x + w / 2, y: t.y },
        { x: t.x + w, y: t.y },
        { x: t.x + w, y: t.y + h },
        { x: t.x, y: t.y + h },
        { x: t.x + w / 2, y: t.y + h / 2 },
      );
    }
    return pts;
  }

  function findNearestSnap(
    wx: number,
    wy: number,
  ): { x: number; y: number } | null {
    const worldThreshold = SNAP_THRESHOLD / zoom;
    if (snapPointsDirty) {
      snapPointsCache = buildSnapPoints();
      snapPointsDirty = false;
    }
    let best: { x: number; y: number } | null = null;
    let bestDist = worldThreshold;
    for (const p of snapPointsCache) {
      const d = Math.hypot(p.x - wx, p.y - wy);
      if (d < bestDist) {
        bestDist = d;
        best = p;
      }
    }
    return best;
  }

  // Persistent offscreen canvas used to composite strokes/shapes with eraser
  // blending. Reused across render() calls to avoid creating a new GPU-backed
  // canvas on every mousemove event.
  let offscreen: HTMLCanvasElement | null = null;
  let offCtx: CanvasRenderingContext2D | null = null;

  // ---------- Rendering ----------
  function render() {
    if (!ctx || !canvasEl) return;
    const w = canvasEl.width;
    const h = canvasEl.height;
    const dpr = window.devicePixelRatio || 1;

    // --- Main canvas: grid only ---
    ctx.clearRect(0, 0, w, h);
    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.save();
    ctx.scale(zoom, zoom);
    ctx.translate(-camX, -camY);
    drawGrid(w / dpr, h / dpr);
    ctx.restore();

    // --- Offscreen canvas: strokes & shapes (eraser only affects these) ---
    // Reuse the same element; resize only when dimensions have changed.
    if (!offscreen || offscreen.width !== w || offscreen.height !== h) {
      if (offscreen) {
        offscreen.width = 0;
        offscreen.height = 0;
      }
      offscreen = document.createElement("canvas");
      offscreen.width = w;
      offscreen.height = h;
      offCtx = offscreen.getContext("2d");
    }
    if (!offCtx) return;
    offCtx.clearRect(0, 0, w, h);
    offCtx.save();
    offCtx.scale(dpr, dpr);
    offCtx.scale(zoom, zoom);
    offCtx.translate(-camX, -camY);

    for (const s of strokes) {
      drawStrokeOn(offCtx, s);
    }
    if (currentStroke) drawStrokeOn(offCtx, currentStroke);

    for (const s of shapes) {
      drawShapeOn(offCtx, s);
    }
    if (currentShape) drawShapeOn(offCtx, currentShape);

    for (const t of textLabels) {
      drawTextOn(offCtx, t);
    }
    offCtx.restore();

    // Composite drawing layer onto main canvas
    ctx.drawImage(offscreen, 0, 0, w, h, 0, 0, w / dpr, h / dpr);

    // Snap point indicators
    if (activeSnap || activeSnapStart) {
      ctx.save();
      ctx.scale(zoom, zoom);
      ctx.translate(-camX, -camY);
      for (const snap of [activeSnapStart, activeSnap]) {
        if (snap) {
          ctx.beginPath();
          ctx.arc(snap.x, snap.y, 4 / zoom, 0, Math.PI * 2);
          ctx.fillStyle = "rgba(59, 130, 246, 0.8)";
          ctx.fill();
          ctx.strokeStyle = "#ffffff";
          ctx.lineWidth = 1.5 / zoom;
          ctx.stroke();
        }
      }
      ctx.restore();
    }

    ctx.restore();
  }

  function drawGrid(w: number, h: number) {
    const step = 40;
    const startX = Math.floor(camX / step) * step;
    const startY = Math.floor(camY / step) * step;
    const endX = camX + w / zoom;
    const endY = camY + h / zoom;

    ctx.strokeStyle =
      theme.current === "dark" ? "rgba(255,255,255,0.06)" : "rgba(0,0,0,0.08)";
    ctx.lineWidth = 1 / zoom;
    ctx.beginPath();
    for (let x = startX; x <= endX; x += step) {
      ctx.moveTo(x, startY);
      ctx.lineTo(x, endY);
    }
    for (let y = startY; y <= endY; y += step) {
      ctx.moveTo(startX, y);
      ctx.lineTo(endX, y);
    }
    ctx.stroke();
  }

  function drawStrokeOn(c: CanvasRenderingContext2D, s: Stroke) {
    if (s.points.length < 2) return;
    c.lineCap = "round";
    c.lineJoin = "round";

    if (s.tool === "eraser") {
      c.globalCompositeOperation = "destination-out";
      c.strokeStyle = "rgba(0,0,0,1)";
    } else {
      c.globalCompositeOperation = "source-over";
      c.strokeStyle = s.color;
    }
    c.lineWidth = s.size;
    c.beginPath();
    c.moveTo(s.points[0].x, s.points[0].y);
    for (let i = 1; i < s.points.length; i++) {
      c.lineTo(s.points[i].x, s.points[i].y);
    }
    c.stroke();
    c.globalCompositeOperation = "source-over";
  }

  function drawShapeOn(c: CanvasRenderingContext2D, s: Shape) {
    c.strokeStyle = s.color;
    c.lineWidth = s.size;
    c.lineCap = "round";
    c.lineJoin = "round";

    if (s.kind === "rect") {
      c.strokeRect(
        Math.min(s.x1, s.x2),
        Math.min(s.y1, s.y2),
        Math.abs(s.x2 - s.x1),
        Math.abs(s.y2 - s.y1),
      );
    } else if (s.kind === "ellipse") {
      const cx = (s.x1 + s.x2) / 2;
      const cy = (s.y1 + s.y2) / 2;
      const rx = Math.abs(s.x2 - s.x1) / 2;
      const ry = Math.abs(s.y2 - s.y1) / 2;
      c.beginPath();
      c.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
      c.stroke();
    } else if (s.kind === "line" || s.kind === "arrow") {
      c.beginPath();
      c.moveTo(s.x1, s.y1);
      c.lineTo(s.x2, s.y2);
      c.stroke();

      if (s.kind === "arrow") {
        const angle = Math.atan2(s.y2 - s.y1, s.x2 - s.x1);
        const headLen = Math.max(10, s.size * 3);
        c.beginPath();
        c.moveTo(s.x2, s.y2);
        c.lineTo(
          s.x2 - headLen * Math.cos(angle - Math.PI / 6),
          s.y2 - headLen * Math.sin(angle - Math.PI / 6),
        );
        c.moveTo(s.x2, s.y2);
        c.lineTo(
          s.x2 - headLen * Math.cos(angle + Math.PI / 6),
          s.y2 - headLen * Math.sin(angle + Math.PI / 6),
        );
        c.stroke();
      }
    }
  }

  function drawTextOn(c: CanvasRenderingContext2D, t: TextLabel) {
    c.font = `${t.fontSize}px Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`;
    c.fillStyle = t.color;
    c.textBaseline = "top";
    c.fillText(t.text, t.x, t.y);
  }

  // ---------- Resize ----------
  function resizeCanvas() {
    if (!canvasEl || !wrapperEl) return;
    const dpr = window.devicePixelRatio || 1;
    const rect = wrapperEl.getBoundingClientRect();
    canvasEl.width = rect.width * dpr;
    canvasEl.height = rect.height * dpr;
    canvasEl.style.width = `${rect.width}px`;
    canvasEl.style.height = `${rect.height}px`;
    ctx = canvasEl.getContext("2d")!;
    render();
  }

  // ---------- Text editing ----------
  function commitText() {
    const et = editingText;
    editingText = null;
    const val = textInputValue.trim();
    textInputValue = "";
    if (et && val) {
      textLabels.push({
        x: et.x,
        y: et.y,
        text: val,
        color: penColor,
        fontSize: textFontSize,
      });
      invalidateSnapPoints();
      render();
      emitChange();
    }
  }

  // ---------- Pointer Handlers ----------
  function handlePointerDown(e: PointerEvent) {
    if (ctxMenu) {
      ctxMenu = null;
      return;
    }

    // Middle-button pan
    if (e.button === 1) {
      e.preventDefault();
      startPan(e);
      return;
    }

    // Space + left click => pan  (handled via keydown flag)
    if (e.button === 0 && (spaceHeld || tool === "hand")) {
      startPan(e);
      return;
    }

    if (e.button !== 0) return;

    const pos = screenToWorld(e.clientX, e.clientY);

    if (tool === "text") {
      // Prevent the browser from focusing the canvas, which would
      // immediately blur the text input we're about to create.
      e.preventDefault();
      if (editingText) {
        blurCommitBlocked = true;
        commitText();
      }
      const local = worldToLocal(pos.x, pos.y);
      editingText = { x: pos.x, y: pos.y, localX: local.x, localY: local.y };
      textInputValue = "";
      blurCommitBlocked = true;
      tick().then(() => {
        textInputEl?.focus();
        blurCommitBlocked = false;
      });
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
      // Shape tools
      isDrawing = true;
      let startX = pos.x,
        startY = pos.y;

      if (tool === "line" || tool === "arrow") {
        const snap = findNearestSnap(pos.x, pos.y);
        if (snap) {
          startX = snap.x;
          startY = snap.y;
          activeSnapStart = snap;
        } else {
          activeSnapStart = null;
        }
      }

      currentShape = {
        kind: tool as Shape["kind"],
        x1: startX,
        y1: startY,
        x2: startX,
        y2: startY,
        color: penColor,
        size: penSize,
      };
      canvasEl.setPointerCapture(e.pointerId);
    }
    render();
  }

  function handlePointerMove(e: PointerEvent) {
    // Update cursor position
    cursorX = e.clientX;
    cursorY = e.clientY;

    if (isPanning) {
      camX = camStartX - (e.clientX - panStartX) / zoom;
      camY = camStartY - (e.clientY - panStartY) / zoom;
      render();
      return;
    }

    if (!isDrawing) return;

    const pos = screenToWorld(e.clientX, e.clientY);

    if (currentStroke) {
      currentStroke.points.push(pos);
      currentStroke = currentStroke; // trigger reactivity
    } else if (currentShape) {
      if (currentShape.kind === "line" || currentShape.kind === "arrow") {
        const snap = findNearestSnap(pos.x, pos.y);
        if (snap) {
          currentShape.x2 = snap.x;
          currentShape.y2 = snap.y;
          activeSnap = snap;
        } else {
          currentShape.x2 = pos.x;
          currentShape.y2 = pos.y;
          activeSnap = null;
        }
      } else {
        currentShape.x2 = pos.x;
        currentShape.y2 = pos.y;
      }
      currentShape = currentShape;
    }
    render();
  }

  function handlePointerUp(e: PointerEvent) {
    if (isPanning) {
      isPanning = false;
      wrapperEl.style.cursor = "";
      emitChange();
      return;
    }

    if (!isDrawing) return;
    isDrawing = false;

    if (currentStroke && currentStroke.points.length >= 2) {
      strokes.push(currentStroke);
    }
    currentStroke = null;

    if (currentShape) {
      const dx = Math.abs(currentShape.x2 - currentShape.x1);
      const dy = Math.abs(currentShape.y2 - currentShape.y1);
      if (dx > 2 || dy > 2) {
        shapes.push(currentShape);
        invalidateSnapPoints();
      }
    }
    currentShape = null;

    activeSnap = null;
    activeSnapStart = null;

    render();
    emitChange();
  }

  function startPan(e: PointerEvent | MouseEvent) {
    isPanning = true;
    panStartX = e.clientX;
    panStartY = e.clientY;
    camStartX = camX;
    camStartY = camY;
    wrapperEl.style.cursor = "grabbing";
  }

  // ---------- Wheel (zoom + scroll-pan) ----------
  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      // Pinch-to-zoom
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
    render();
  }

  // ---------- Keyboard ----------
  let spaceHeld = false;
  function handleKeyDown(e: KeyboardEvent) {
    if (e.code === "Space" && !e.repeat) {
      spaceHeld = true;
      if (!isDrawing) wrapperEl.style.cursor = "grab";
    }
    // Undo: Ctrl/Cmd+Z
    if ((e.ctrlKey || e.metaKey) && e.key === "z" && !e.shiftKey) {
      e.preventDefault();
      if (strokes.length > 0) {
        strokes.pop();
      } else if (shapes.length > 0) {
        shapes.pop();
        invalidateSnapPoints();
      } else if (textLabels.length > 0) {
        textLabels.pop();
        invalidateSnapPoints();
      }
      render();
      emitChange();
    }
  }

  function handleKeyUp(e: KeyboardEvent) {
    if (e.code === "Space") {
      spaceHeld = false;
      if (!isPanning) wrapperEl.style.cursor = "";
    }
  }

  // ---------- Context Menu ----------
  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    ctxMenu = { x: e.clientX, y: e.clientY };
  }

  function closeContextMenu() {
    ctxMenu = null;
  }

  // ---------- Lifecycle ----------
  let resizeObserver: ResizeObserver;

  onMount(async () => {
    // Always read the latest data from disk to survive tab switches
    try {
      const bytes = await readFileBytes(filePath);
      const fileData = new TextDecoder().decode(bytes);
      if (fileData) {
        deserialize(fileData);
      } else {
        deserialize(initialData);
      }
    } catch {
      deserialize(initialData);
    }
    resizeCanvas();
    resizeObserver = new ResizeObserver(() => resizeCanvas());
    resizeObserver.observe(wrapperEl);
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    if (offscreen) {
      offscreen.width = 0;
      offscreen.height = 0;
      offscreen = null;
      offCtx = null;
    }
  });

  // Reactive current size for display
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

  <!-- Toolbar -->
  <div class="toolbar">
    <div class="toolbar-group">
      <button
        class="tool-btn"
        class:active={tool === "hand"}
        onclick={() => (tool = "hand")}
        title={m.canvas_hand()}
      >
        <Hand size={16} />
      </button>
      <button
        class="tool-btn"
        class:active={tool === "pen"}
        onclick={() => (tool = "pen")}
        title={m.canvas_pen()}
      >
        <Pencil size={16} />
      </button>
      <button
        class="tool-btn"
        class:active={tool === "eraser"}
        onclick={() => (tool = "eraser")}
        title={m.canvas_eraser()}
      >
        <Eraser size={16} />
      </button>
      <span class="toolbar-sep"></span>
      <button
        class="tool-btn"
        class:active={tool === "rect"}
        onclick={() => (tool = "rect")}
        title={m.canvas_rect()}
      >
        <Square size={16} />
      </button>
      <button
        class="tool-btn"
        class:active={tool === "ellipse"}
        onclick={() => (tool = "ellipse")}
        title={m.canvas_ellipse()}
      >
        <Circle size={16} />
      </button>
      <button
        class="tool-btn"
        class:active={tool === "line"}
        onclick={() => (tool = "line")}
        title={m.canvas_line()}
      >
        <Minus size={16} />
      </button>
      <button
        class="tool-btn"
        class:active={tool === "arrow"}
        onclick={() => (tool = "arrow")}
        title={m.canvas_arrow()}
      >
        <ArrowUpRight size={16} />
      </button>
      <button
        class="tool-btn"
        class:active={tool === "text"}
        onclick={() => (tool = "text")}
        title={m.canvas_text()}
      >
        <Type size={16} />
      </button>
    </div>

    <span class="toolbar-sep"></span>

    <!-- Color swatches -->
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

    <!-- Size -->
    <div class="toolbar-group size-group">
      <label class="size-label" for="canvas-size-slider">{currentSize}px</label>
      <input
        id="canvas-size-slider"
        type="range"
        min={tool === "text" ? 8 : 1}
        max={tool === "eraser" ? 60 : tool === "text" ? 72 : 30}
        value={currentSize}
        oninput={(e) => {
          const v = Number(e.currentTarget.value);
          if (tool === "eraser") eraserSize = v;
          else if (tool === "text") textFontSize = v;
          else penSize = v;
        }}
        class="size-slider"
      />
    </div>

    <span class="toolbar-sep"></span>

    <!-- Zoom -->
    <div class="toolbar-group">
      <button
        class="tool-btn"
        onclick={() => {
          zoom = Math.min(5, zoom * 1.2);
          render();
        }}
        title="Zoom in"
      >
        <ZoomIn size={14} />
      </button>
      <span class="zoom-label">{Math.round(zoom * 100)}%</span>
      <button
        class="tool-btn"
        onclick={() => {
          zoom = Math.max(0.1, zoom / 1.2);
          render();
        }}
        title="Zoom out"
      >
        <ZoomOut size={14} />
      </button>
      <button
        class="tool-btn"
        onclick={() => {
          zoom = 1;
          camX = 0;
          camY = 0;
          render();
        }}
        title="Reset view"
      >
        <RotateCcw size={14} />
      </button>
    </div>
  </div>

  <!-- Right-click context menu -->
  {#if ctxMenu}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="ctx-backdrop" onmousedown={closeContextMenu}></div>
    <div
      class="ctx-menu"
      style:left={`${ctxMenu.x}px`}
      style:top={`${ctxMenu.y}px`}
      role="menu"
    >
      <div class="ctx-section-label">{m.canvas_tool()}</div>
      <button
        class="ctx-item"
        class:active={tool === "pen"}
        onclick={() => {
          tool = "pen";
          closeContextMenu();
        }}
        role="menuitem"
      >
        {m.canvas_pen()}
      </button>
      <button
        class="ctx-item"
        class:active={tool === "eraser"}
        onclick={() => {
          tool = "eraser";
          closeContextMenu();
        }}
        role="menuitem"
      >
        {m.canvas_eraser()}
      </button>
      <button
        class="ctx-item"
        class:active={tool === "text"}
        onclick={() => {
          tool = "text";
          closeContextMenu();
        }}
        role="menuitem"
      >
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
            onclick={() => {
              penColor = c;
              closeContextMenu();
            }}
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
            onclick={() => {
              if (tool === "eraser") eraserSize = s;
              else if (tool === "text") textFontSize = s;
              else penSize = s;
              closeContextMenu();
            }}
          >
            {s}
          </button>
        {/each}
      </div>
      <div class="ctx-sep"></div>

      <button
        class="ctx-item danger"
        onclick={() => {
          strokes = [];
          shapes = [];
          textLabels = [];
          render();
          emitChange();
          closeContextMenu();
        }}
        role="menuitem"
      >
        {m.canvas_clear_all()}
      </button>
    </div>
  {/if}

  <!-- Text input overlay -->
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
        if (e.key === "Escape") {
          editingText = null;
          textInputValue = "";
        }
        e.stopPropagation();
      }}
      onblur={() => {
        if (blurCommitBlocked) {
          blurCommitBlocked = false;
          return;
        }
        commitText();
      }}
    />
  {/if}

  <!-- Custom cursor -->
  {#if showCursor && !isPanning && !ctxMenu && tool !== "text"}
    <div
      class="custom-cursor"
      style:left={`${cursorX}px`}
      style:top={`${cursorY}px`}
      style:width={`${currentSize * zoom}px`}
      style:height={`${currentSize * zoom}px`}
      style:border-color={tool === "eraser"
        ? "rgba(255,255,255,0.5)"
        : penColor}
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

  /* --- Toolbar --- */
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
    transition:
      background 0.1s,
      color 0.1s;
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

  /* Colors */
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
    transition:
      border-color 0.1s,
      transform 0.1s;
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

  /* Size slider */
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

  /* --- Context Menu --- */
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
    transition:
      background 0.1s,
      color 0.1s;
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
    transition:
      background 0.1s,
      color 0.1s,
      border-color 0.1s;
  }

  .ctx-size-btn:hover {
    background: var(--bg-hover);
  }
  .ctx-size-btn.active {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--bg-tertiary);
  }

  /* --- Text Tool --- */
  .canvas-wrapper:global(.tool-text) {
    cursor: text;
  }

  .text-input-overlay {
    position: absolute;
    z-index: 20;
    background: transparent;
    border: 1px dashed var(--accent);
    outline: none;
    font-family:
      Inter,
      -apple-system,
      BlinkMacSystemFont,
      "Segoe UI",
      sans-serif;
    padding: 2px 4px;
    min-width: 100px;
    caret-color: var(--accent);
    border-radius: 2px;
  }

  /* --- Custom Cursor --- */
  .custom-cursor {
    position: fixed;
    pointer-events: none;
    border: 1.5px solid;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    z-index: 50;
    transition:
      width 0.1s,
      height 0.1s;
    min-width: 4px;
    min-height: 4px;
  }
</style>
