export type Tool = 'hand' | 'pen' | 'eraser' | 'rect' | 'ellipse' | 'line' | 'arrow' | 'text';

export type ShapeKind = 'rect' | 'ellipse' | 'line' | 'arrow';

export type StrokeWidth = 'thin' | 'medium' | 'thick';

export interface Point {
	x: number;
	y: number;
}

export interface Stroke {
	points: Point[];
	color: string;
	size: number;
	tool: 'pen' | 'eraser';
}

export interface Shape {
	kind: ShapeKind;
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

export const DEFAULT_PEN_COLOR = '#3b82f6';

export const colorPresets = ['#ef4444', '#f59e0b', '#22c55e', '#3b82f6', '#a855f7', '#6b7280'];

export const shapeKinds: ShapeKind[] = ['rect', 'ellipse', 'line', 'arrow'];

export function isShapeTool(tool: Tool): tool is ShapeKind {
	return shapeKinds.includes(tool as ShapeKind);
}

export const strokeWidths: StrokeWidth[] = ['thin', 'medium', 'thick'];

const WIDTH_VALUES: Record<Tool, readonly [number, number, number]> = {
	hand: [1.5, 3, 6],
	pen: [1.5, 3, 6],
	rect: [1.5, 3, 6],
	ellipse: [1.5, 3, 6],
	line: [1.5, 3, 6],
	arrow: [1.5, 3, 6],
	eraser: [12, 24, 48],
	text: [16, 24, 40]
};

export function widthOptions(tool: Tool): { width: StrokeWidth; value: number }[] {
	return strokeWidths.map((width, i) => ({ width, value: WIDTH_VALUES[tool][i] }));
}

export function nearestWidth(tool: Tool, size: number): StrokeWidth {
	const options = widthOptions(tool);
	let best = options[0];
	for (const option of options) {
		if (Math.abs(option.value - size) < Math.abs(best.value - size)) best = option;
	}
	return best.width;
}
