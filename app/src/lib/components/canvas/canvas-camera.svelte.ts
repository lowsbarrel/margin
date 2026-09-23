import type { Point } from '$lib/canvas/types';

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 5;

function clampZoom(zoom: number) {
	return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

export class CanvasCamera {
	x = $state(0);
	y = $state(0);
	zoom = $state(1);
	panning = $state(false);

	private grabFrom = { x: 0, y: 0 };
	private grabStart = { x: 0, y: 0 };

	toWorld(rect: DOMRect, screenX: number, screenY: number): Point {
		return {
			x: (screenX - rect.left) / this.zoom + this.x,
			y: (screenY - rect.top) / this.zoom + this.y
		};
	}

	toScreen(worldX: number, worldY: number): Point {
		return { x: (worldX - this.x) * this.zoom, y: (worldY - this.y) * this.zoom };
	}

	restore(x: number, y: number, zoom: number) {
		this.x = x;
		this.y = y;
		this.zoom = zoom;
	}

	reset() {
		this.restore(0, 0, 1);
	}

	setZoom(zoom: number) {
		this.zoom = clampZoom(zoom);
	}

	beginPan(clientX: number, clientY: number) {
		this.panning = true;
		this.grabFrom = { x: clientX, y: clientY };
		this.grabStart = { x: this.x, y: this.y };
	}

	movePan(clientX: number, clientY: number) {
		this.x = this.grabStart.x - (clientX - this.grabFrom.x) / this.zoom;
		this.y = this.grabStart.y - (clientY - this.grabFrom.y) / this.zoom;
	}

	endPan() {
		this.panning = false;
	}

	panBy(deltaX: number, deltaY: number) {
		this.x += deltaX / this.zoom;
		this.y += deltaY / this.zoom;
	}

	zoomAt(rect: DOMRect, clientX: number, clientY: number, factor: number) {
		const next = clampZoom(this.zoom * factor);
		const worldX = (clientX - rect.left) / this.zoom + this.x;
		const worldY = (clientY - rect.top) / this.zoom + this.y;
		this.x = worldX - (clientX - rect.left) / next;
		this.y = worldY - (clientY - rect.top) / next;
		this.zoom = next;
	}
}
