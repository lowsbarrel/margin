import type { Stroke, Shape, TextLabel, CanvasData } from "./types";

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

export function deserialize(raw: string): CanvasData | null {
  if (!raw) return null;
  try {
    const data = JSON.parse(raw);
    return {
      strokes: data.strokes ?? [],
      shapes: data.shapes ?? [],
      textLabels: data.textLabels ?? [],
      camX: data.camX ?? 0,
      camY: data.camY ?? 0,
      zoom: data.zoom ?? 1,
    };
  } catch {
    console.warn("Canvas: could not parse saved drawing data");
    return null;
  }
}
