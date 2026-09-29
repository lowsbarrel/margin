import type { CanvasData, Shape, Stroke, TextLabel } from '$lib/canvas/types';

export type UndoKind = 'stroke' | 'shape' | 'label';

export class CanvasScene {
	strokes: Stroke[] = [];
	shapes: Shape[] = [];
	labels: TextLabel[] = [];

	load(data: CanvasData) {
		this.strokes = data.strokes;
		this.shapes = data.shapes;
		this.labels = data.textLabels;
	}

	undo(): UndoKind | null {
		if (this.strokes.length > 0) {
			this.strokes.pop();
			return 'stroke';
		}
		if (this.shapes.length > 0) {
			this.shapes.pop();
			return 'shape';
		}
		if (this.labels.length > 0) {
			this.labels.pop();
			return 'label';
		}
		return null;
	}

	clear() {
		this.strokes = [];
		this.shapes = [];
		this.labels = [];
	}
}
