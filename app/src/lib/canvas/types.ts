export type Tool =
  | "hand"
  | "pen"
  | "eraser"
  | "rect"
  | "ellipse"
  | "line"
  | "arrow"
  | "text";

export interface Stroke {
  points: { x: number; y: number }[];
  color: string;
  size: number;
  tool: "pen" | "eraser";
}

export interface Shape {
  kind: "rect" | "ellipse" | "line" | "arrow";
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
  size: number;
}

export interface TextLabel {
  x: number;
  y: number;
  text: string;
  color: string;
  fontSize: number;
}

export interface CanvasData {
  strokes: Stroke[];
  shapes: Shape[];
  textLabels: TextLabel[];
  camX: number;
  camY: number;
  zoom: number;
}

export const colorPresets = [
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

export const sizePresets = [1, 2, 3, 5, 8, 12, 20];
