import { ArrowUpRight, Circle, Minus, Square } from '@lucide/svelte';
import * as m from '$lib/paraglide/messages.js';
import type { ShapeKind } from '$lib/canvas/types';

export const shapeIcons = { rect: Square, ellipse: Circle, line: Minus, arrow: ArrowUpRight };

export const shapeLabels: Record<ShapeKind, () => string> = {
	rect: m.canvas_rect,
	ellipse: m.canvas_ellipse,
	line: m.canvas_line,
	arrow: m.canvas_arrow
};
