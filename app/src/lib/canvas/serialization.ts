import type { Point, Stroke, Shape, TextLabel, CanvasData } from "./types";

export function serialize(
  strokes: Stroke[],
  shapes: Shape[],
  textLabels: TextLabel[],
  camX: number,
  camY: number,
  zoom: number,
): string {
  return JSON.stringify({ strokes, shapes, textLabels, camX, camY, zoom });
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null;
}

function isFiniteNumber(v: unknown): v is number {
  return typeof v === "number" && Number.isFinite(v);
}

function asNumber(v: unknown, fallback: number): number {
  return isFiniteNumber(v) ? v : fallback;
}

function isPoint(v: unknown): v is Point {
  return isRecord(v) && isFiniteNumber(v.x) && isFiniteNumber(v.y);
}

function parseStrokes(raw: unknown): Stroke[] {
  if (!Array.isArray(raw)) return [];
  const out: Stroke[] = [];
  for (const item of raw) {
    if (!isRecord(item)) continue;
    if (!Array.isArray(item.points)) continue;
    const points = item.points.filter(isPoint);
    if (points.length === 0) continue;
    const tool = item.tool === "eraser" ? "eraser" : "pen";
    out.push({
      points,
      color: typeof item.color === "string" ? item.color : "#ffffff",
      size: asNumber(item.size, 3),
      tool,
    });
  }
  return out;
}

const SHAPE_KINDS: ReadonlySet<Shape["kind"]> = new Set([
  "rect",
  "ellipse",
  "line",
  "arrow",
]);

function parseShapes(raw: unknown): Shape[] {
  if (!Array.isArray(raw)) return [];
  const out: Shape[] = [];
  for (const item of raw) {
    if (!isRecord(item)) continue;
    if (!SHAPE_KINDS.has(item.kind as Shape["kind"])) continue;
    if (
      !isFiniteNumber(item.x1) ||
      !isFiniteNumber(item.y1) ||
      !isFiniteNumber(item.x2) ||
      !isFiniteNumber(item.y2)
    ) {
      continue;
    }
    out.push({
      kind: item.kind as Shape["kind"],
      x1: item.x1,
      y1: item.y1,
      x2: item.x2,
      y2: item.y2,
      color: typeof item.color === "string" ? item.color : "#ffffff",
      size: asNumber(item.size, 3),
    });
  }
  return out;
}

function parseTextLabels(raw: unknown): TextLabel[] {
  if (!Array.isArray(raw)) return [];
  const out: TextLabel[] = [];
  for (const item of raw) {
    if (!isRecord(item)) continue;
    if (typeof item.text !== "string") continue;
    if (!isFiniteNumber(item.x) || !isFiniteNumber(item.y)) continue;
    out.push({
      x: item.x,
      y: item.y,
      text: item.text,
      color: typeof item.color === "string" ? item.color : "#ffffff",
      fontSize: asNumber(item.fontSize, 16),
    });
  }
  return out;
}

export function deserialize(raw: string): CanvasData | null {
  if (!raw) return null;
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    console.warn("Canvas: could not parse saved drawing data");
    return null;
  }
  if (!isRecord(data)) return null;
  return {
    strokes: parseStrokes(data.strokes),
    shapes: parseShapes(data.shapes),
    textLabels: parseTextLabels(data.textLabels),
    camX: asNumber(data.camX, 0),
    camY: asNumber(data.camY, 0),
    zoom: asNumber(data.zoom, 1),
  };
}
