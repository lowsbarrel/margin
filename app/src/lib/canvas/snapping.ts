import type { Shape, TextLabel } from "./types";

const textWidthCache = new Map<string, number>();

export function measureTextWidth(
  ctx: CanvasRenderingContext2D | null,
  text: string,
  fontSize: number,
): number {
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

export function buildSnapPoints(
  shapes: Shape[],
  textLabels: TextLabel[],
  ctx: CanvasRenderingContext2D | null,
): { x: number; y: number }[] {
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
    const w = measureTextWidth(ctx, t.text, t.fontSize);
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

export class SnapCache {
  private cache: { x: number; y: number }[] = [];
  private dirty = true;
  private threshold: number;

  constructor(threshold = 12) {
    this.threshold = threshold;
  }

  invalidate() {
    this.dirty = true;
  }

  findNearest(
    wx: number,
    wy: number,
    zoom: number,
    shapes: Shape[],
    textLabels: TextLabel[],
    ctx: CanvasRenderingContext2D | null,
  ): { x: number; y: number } | null {
    const worldThreshold = this.threshold / zoom;
    if (this.dirty) {
      this.cache = buildSnapPoints(shapes, textLabels, ctx);
      this.dirty = false;
    }
    let best: { x: number; y: number } | null = null;
    let bestDist = worldThreshold;
    for (const p of this.cache) {
      const d = Math.hypot(p.x - wx, p.y - wy);
      if (d < bestDist) {
        bestDist = d;
        best = p;
      }
    }
    return best;
  }
}
