import { theme } from '$lib/stores/theme.svelte';
import type { Point, Shape, Stroke, Tool } from '$lib/canvas/types';
import { render as renderCanvas, type RenderOptions } from '$lib/canvas/renderer';
import { isFormControlFocused } from '$lib/utils/modal';
import type { SnapCache } from '$lib/canvas/snapping';
import type { CanvasCamera } from './canvas-camera.svelte';
import type { CanvasPersist } from './canvas-persist';
import type { CanvasScene } from './canvas-scene';
import type { ToolSizes } from './tool-sizes.svelte';

export interface CanvasSurfaceDeps {
	canvas: () => HTMLCanvasElement;
	wrapper: () => HTMLDivElement;
	camera: CanvasCamera;
	scene: CanvasScene;
	sizes: ToolSizes;
	snapCache: SnapCache;
	persist: CanvasPersist;
	tool: () => Tool;
	penColor: () => string;
	menuOpen: () => boolean;
	closeMenu: () => void;
	beginText: (point: Point) => void;
}

export function createCanvasSurface(deps: CanvasSurfaceDeps) {
	let ctx: CanvasRenderingContext2D;
	let currentStroke: Stroke | null = null;
	let currentShape: Shape | null = null;
	let isDrawing = false;
	let activeSnap: Point | null = null;
	let activeSnapStart: Point | null = null;
	let spaceHeld = false;
	let needsRender = false;
	let frame = 0;

	let cursorX = $state(0);
	let cursorY = $state(0);

	const offscreen: { canvas: HTMLCanvasElement | null; ctx: CanvasRenderingContext2D | null } = {
		canvas: null,
		ctx: null
	};

	function draw() {
		const canvasEl = deps.canvas();
		if (!ctx || !canvasEl) return;
		const opts: RenderOptions = {
			ctx,
			canvasEl,
			camX: deps.camera.x,
			camY: deps.camera.y,
			zoom: deps.camera.zoom,
			isDark: theme.current === 'dark',
			strokes: deps.scene.strokes,
			shapes: deps.scene.shapes,
			textLabels: deps.scene.labels,
			currentStroke,
			currentShape,
			activeSnap,
			activeSnapStart
		};
		renderCanvas(opts, offscreen);
	}

	function frameStep() {
		frame = 0;
		if (!needsRender) return;
		needsRender = false;
		draw();
	}

	function scheduleRender() {
		needsRender = true;
		if (frame === 0) frame = requestAnimationFrame(frameStep);
	}

	function resize() {
		const canvasEl = deps.canvas();
		const wrapperEl = deps.wrapper();
		if (!canvasEl || !wrapperEl) return;
		const dpr = window.devicePixelRatio || 1;
		const rect = wrapperEl.getBoundingClientRect();
		canvasEl.width = rect.width * dpr;
		canvasEl.height = rect.height * dpr;
		canvasEl.style.width = `${rect.width}px`;
		canvasEl.style.height = `${rect.height}px`;
		ctx = canvasEl.getContext('2d')!;
		draw();
	}

	function toWorld(sx: number, sy: number): Point {
		return deps.camera.toWorld(deps.canvas().getBoundingClientRect(), sx, sy);
	}

	function startPan(e: PointerEvent) {
		deps.camera.beginPan(e.clientX, e.clientY);
		deps.wrapper().style.cursor = 'grabbing';
	}

	function pointerDown(e: PointerEvent) {
		if (deps.menuOpen()) {
			deps.closeMenu();
			return;
		}
		if (e.button === 1) {
			e.preventDefault();
			startPan(e);
			return;
		}
		const tool = deps.tool();
		if (e.button === 0 && (spaceHeld || tool === 'hand')) {
			startPan(e);
			return;
		}
		if (e.button !== 0) return;

		const pos = toWorld(e.clientX, e.clientY);

		if (tool === 'text') {
			e.preventDefault();
			deps.beginText(pos);
			return;
		}

		if (tool === 'pen' || tool === 'eraser') {
			isDrawing = true;
			currentStroke = {
				points: [pos],
				color: tool === 'eraser' ? '#000' : deps.penColor(),
				size: deps.sizes.current(tool),
				tool
			};
			deps.canvas().setPointerCapture(e.pointerId);
		} else {
			isDrawing = true;
			let startX = pos.x;
			let startY = pos.y;
			if (tool === 'line' || tool === 'arrow') {
				const snap = deps.snapCache.findNearest(
					pos.x,
					pos.y,
					deps.camera.zoom,
					deps.scene.shapes,
					deps.scene.labels,
					ctx
				);
				if (snap) {
					startX = snap.x;
					startY = snap.y;
					activeSnapStart = snap;
				} else activeSnapStart = null;
			}
			currentShape = {
				kind: tool as Shape['kind'],
				x1: startX,
				y1: startY,
				x2: startX,
				y2: startY,
				color: deps.penColor(),
				size: deps.sizes.current(tool)
			};
			deps.canvas().setPointerCapture(e.pointerId);
		}
		scheduleRender();
	}

	function pointerMove(e: PointerEvent) {
		cursorX = e.clientX;
		cursorY = e.clientY;
		if (deps.camera.panning) {
			deps.camera.movePan(e.clientX, e.clientY);
			scheduleRender();
			return;
		}
		if (!isDrawing) return;
		if (currentStroke) {
			const coalesced = typeof e.getCoalescedEvents === 'function' ? e.getCoalescedEvents() : [];
			const events = coalesced.length > 0 ? coalesced : [e];
			for (const ev of events) {
				currentStroke.points.push(toWorld(ev.clientX, ev.clientY));
			}
		} else if (currentShape) {
			const pos = toWorld(e.clientX, e.clientY);
			if (currentShape.kind === 'line' || currentShape.kind === 'arrow') {
				const snap = deps.snapCache.findNearest(
					pos.x,
					pos.y,
					deps.camera.zoom,
					deps.scene.shapes,
					deps.scene.labels,
					ctx
				);
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
		}
		scheduleRender();
	}

	function pointerUp(_e: PointerEvent) {
		if (deps.camera.panning) {
			deps.camera.endPan();
			deps.wrapper().style.cursor = '';
			deps.persist.schedule();
			return;
		}
		if (!isDrawing) return;
		isDrawing = false;
		if (currentStroke && currentStroke.points.length >= 2) deps.scene.strokes.push(currentStroke);
		currentStroke = null;
		if (currentShape) {
			const dx = Math.abs(currentShape.x2 - currentShape.x1);
			const dy = Math.abs(currentShape.y2 - currentShape.y1);
			if (dx > 2 || dy > 2) {
				deps.scene.shapes.push(currentShape);
				deps.snapCache.invalidate();
			}
		}
		currentShape = null;
		activeSnap = null;
		activeSnapStart = null;
		scheduleRender();
		deps.persist.schedule();
	}

	function wheel(e: WheelEvent) {
		e.preventDefault();
		if (e.ctrlKey || e.metaKey) {
			const rect = deps.canvas().getBoundingClientRect();
			deps.camera.zoomAt(rect, e.clientX, e.clientY, e.deltaY > 0 ? 0.9 : 1.1);
		} else {
			deps.camera.panBy(e.deltaX, e.deltaY);
		}
		scheduleRender();
		deps.persist.schedule();
	}

	function keyDown(e: KeyboardEvent) {
		if (isFormControlFocused()) return;
		if (e.code === 'Space' && !e.repeat) {
			spaceHeld = true;
			if (!isDrawing) deps.wrapper().style.cursor = 'grab';
		}
		if ((e.ctrlKey || e.metaKey) && e.key === 'z' && !e.shiftKey) {
			e.preventDefault();
			const undone = deps.scene.undo();
			if (undone === 'shape' || undone === 'label') deps.snapCache.invalidate();
			scheduleRender();
			deps.persist.schedule();
		}
	}

	function keyUp(e: KeyboardEvent) {
		if (e.code === 'Space') {
			spaceHeld = false;
			if (!deps.camera.panning) deps.wrapper().style.cursor = '';
		}
	}

	function stop() {
		if (frame !== 0) cancelAnimationFrame(frame);
		frame = 0;
		needsRender = false;
		if (offscreen.canvas) {
			offscreen.canvas.width = 0;
			offscreen.canvas.height = 0;
			offscreen.canvas = null;
			offscreen.ctx = null;
		}
	}

	return {
		pointerDown,
		pointerMove,
		pointerUp,
		wheel,
		keyDown,
		keyUp,
		resize,
		scheduleRender,
		stop,
		get cursorX() {
			return cursorX;
		},
		get cursorY() {
			return cursorY;
		}
	};
}
