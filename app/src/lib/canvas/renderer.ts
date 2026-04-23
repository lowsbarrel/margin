import type { Stroke, Shape, TextLabel } from "./types";

export function drawGrid(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  camX: number,
  camY: number,
  zoom: number,
  isDark: boolean,
) {
  const step = 40;
  const startX = Math.floor(camX / step) * step;
  const startY = Math.floor(camY / step) * step;
  const endX = camX + w / zoom;
  const endY = camY + h / zoom;

  ctx.strokeStyle = isDark ? "rgba(255,255,255,0.06)" : "rgba(0,0,0,0.08)";
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

export function drawStrokeOn(c: CanvasRenderingContext2D, s: Stroke) {
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

export function drawShapeOn(c: CanvasRenderingContext2D, s: Shape) {
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

export function drawTextOn(c: CanvasRenderingContext2D, t: TextLabel) {
  c.font = `${t.fontSize}px Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`;
  c.fillStyle = t.color;
  c.textBaseline = "top";
  c.fillText(t.text, t.x, t.y);
}

export interface RenderOptions {
  ctx: CanvasRenderingContext2D;
  canvasEl: HTMLCanvasElement;
  camX: number;
  camY: number;
  zoom: number;
  isDark: boolean;
  strokes: Stroke[];
  shapes: Shape[];
  textLabels: TextLabel[];
  currentStroke: Stroke | null;
  currentShape: Shape | null;
  activeSnap: { x: number; y: number } | null;
  activeSnapStart: { x: number; y: number } | null;
}

/**
 * Composites the full canvas scene: grid → drawing layer (offscreen) → snap indicators.
 * Re-uses offscreen to avoid creating new GPU-backed canvases on every frame.
 */
export function render(
  opts: RenderOptions,
  offscreenRef: { canvas: HTMLCanvasElement | null; ctx: CanvasRenderingContext2D | null },
) {
  const { ctx, canvasEl, camX, camY, zoom, isDark } = opts;
  const w = canvasEl.width;
  const h = canvasEl.height;
  const dpr = window.devicePixelRatio || 1;

  ctx.clearRect(0, 0, w, h);
  ctx.save();
  ctx.scale(dpr, dpr);
  ctx.save();
  ctx.scale(zoom, zoom);
  ctx.translate(-camX, -camY);
  drawGrid(ctx, w / dpr, h / dpr, camX, camY, zoom, isDark);
  ctx.restore();

  // Offscreen canvas for strokes/shapes (eraser compositing)
  if (!offscreenRef.canvas || offscreenRef.canvas.width !== w || offscreenRef.canvas.height !== h) {
    if (offscreenRef.canvas) {
      offscreenRef.canvas.width = 0;
      offscreenRef.canvas.height = 0;
    }
    offscreenRef.canvas = document.createElement("canvas");
    offscreenRef.canvas.width = w;
    offscreenRef.canvas.height = h;
    offscreenRef.ctx = offscreenRef.canvas.getContext("2d");
  }
  const offCtx = offscreenRef.ctx;
  if (!offCtx) { ctx.restore(); return; }

  offCtx.clearRect(0, 0, w, h);
  offCtx.save();
  offCtx.scale(dpr, dpr);
  offCtx.scale(zoom, zoom);
  offCtx.translate(-camX, -camY);

  for (const s of opts.strokes) drawStrokeOn(offCtx, s);
  if (opts.currentStroke) drawStrokeOn(offCtx, opts.currentStroke);

  for (const s of opts.shapes) drawShapeOn(offCtx, s);
  if (opts.currentShape) drawShapeOn(offCtx, opts.currentShape);

  for (const t of opts.textLabels) drawTextOn(offCtx, t);
  offCtx.restore();

  ctx.drawImage(offscreenRef.canvas, 0, 0, w, h, 0, 0, w / dpr, h / dpr);

  // Snap indicators
  if (opts.activeSnap || opts.activeSnapStart) {
    ctx.save();
    ctx.scale(zoom, zoom);
    ctx.translate(-camX, -camY);
    for (const snap of [opts.activeSnapStart, opts.activeSnap]) {
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
