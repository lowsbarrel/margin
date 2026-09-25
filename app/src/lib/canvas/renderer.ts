import type { Point, Stroke, Shape, TextLabel } from './types';
import { INK_COLOR } from './types';

let tokenCache: { isDark: boolean; grid: string; ink: string } | null = null;

function themeTokens(isDark: boolean) {
	if (tokenCache && tokenCache.isDark === isDark) return tokenCache;
	const style = getComputedStyle(document.documentElement);
	const grid = style.getPropertyValue('--color-canvas-grid').trim();
	const ink = style.getPropertyValue('--color-canvas-ink').trim();
	tokenCache = {
		isDark,
		grid: grid || (isDark ? 'hsl(0 0% 100% / 14%)' : 'hsl(0 0% 25% / 12%)'),
		ink: ink || (isDark ? 'hsl(0 0% 96%)' : 'hsl(0 0% 12%)')
	};
	return tokenCache;
}

const NEAR_WHITE_LUMINANCE = 0.85;

function isNearWhite(color: string): boolean {
	const hex = color.trim().replace('#', '');
	const full =
		hex.length === 3
			? hex
					.split('')
					.map((c) => c + c)
					.join('')
			: hex;
	if (!/^[0-9a-f]{6}$/i.test(full)) return false;
	const r = parseInt(full.slice(0, 2), 16) / 255;
	const g = parseInt(full.slice(2, 4), 16) / 255;
	const b = parseInt(full.slice(4, 6), 16) / 255;
	return 0.2126 * r + 0.7152 * g + 0.0722 * b > NEAR_WHITE_LUMINANCE;
}

// Drawings made before ink existed are full of near-white strokes: paint them as ink so they stay legible in both themes.
function resolveColor(color: string, isDark: boolean): string {
	if (color === INK_COLOR || isNearWhite(color)) return themeTokens(isDark).ink;
	return color;
}

function drawGrid(
	ctx: CanvasRenderingContext2D,
	w: number,
	h: number,
	camX: number,
	camY: number,
	zoom: number,
	isDark: boolean
) {
	const baseStep = 40;
	const step = baseStep * Math.pow(2, Math.round(Math.log2(1 / zoom)));
	const startX = Math.floor(camX / step) * step;
	const startY = Math.floor(camY / step) * step;
	const endX = camX + w / zoom;
	const endY = camY + h / zoom;
	const radius = 1 / zoom;

	ctx.fillStyle = themeTokens(isDark).grid;
	ctx.beginPath();
	for (let x = startX; x <= endX; x += step) {
		for (let y = startY; y <= endY; y += step) {
			ctx.moveTo(x + radius, y);
			ctx.arc(x, y, radius, 0, Math.PI * 2);
		}
	}
	ctx.fill();
}

function drawStrokeOn(c: CanvasRenderingContext2D, s: Stroke, isDark: boolean) {
	if (s.points.length < 2) return;
	c.lineCap = 'round';
	c.lineJoin = 'round';

	if (s.tool === 'eraser') {
		c.globalCompositeOperation = 'destination-out';
		c.strokeStyle = 'rgba(0,0,0,1)';
	} else {
		c.globalCompositeOperation = 'source-over';
		c.strokeStyle = resolveColor(s.color, isDark);
	}
	c.lineWidth = s.size;
	c.beginPath();
	c.moveTo(s.points[0].x, s.points[0].y);
	for (let i = 1; i < s.points.length; i++) {
		c.lineTo(s.points[i].x, s.points[i].y);
	}
	c.stroke();
	c.globalCompositeOperation = 'source-over';
}

function drawShapeOn(c: CanvasRenderingContext2D, s: Shape, isDark: boolean) {
	c.strokeStyle = resolveColor(s.color, isDark);
	c.lineWidth = s.size;
	c.lineCap = 'round';
	c.lineJoin = 'round';

	if (s.kind === 'rect') {
		c.strokeRect(
			Math.min(s.x1, s.x2),
			Math.min(s.y1, s.y2),
			Math.abs(s.x2 - s.x1),
			Math.abs(s.y2 - s.y1)
		);
	} else if (s.kind === 'ellipse') {
		const cx = (s.x1 + s.x2) / 2;
		const cy = (s.y1 + s.y2) / 2;
		const rx = Math.abs(s.x2 - s.x1) / 2;
		const ry = Math.abs(s.y2 - s.y1) / 2;
		c.beginPath();
		c.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
		c.stroke();
	} else if (s.kind === 'line' || s.kind === 'arrow') {
		c.beginPath();
		c.moveTo(s.x1, s.y1);
		c.lineTo(s.x2, s.y2);
		c.stroke();

		if (s.kind === 'arrow') {
			const angle = Math.atan2(s.y2 - s.y1, s.x2 - s.x1);
			const headLen = Math.max(10, s.size * 3);
			c.beginPath();
			c.moveTo(s.x2, s.y2);
			c.lineTo(
				s.x2 - headLen * Math.cos(angle - Math.PI / 6),
				s.y2 - headLen * Math.sin(angle - Math.PI / 6)
			);
			c.moveTo(s.x2, s.y2);
			c.lineTo(
				s.x2 - headLen * Math.cos(angle + Math.PI / 6),
				s.y2 - headLen * Math.sin(angle + Math.PI / 6)
			);
			c.stroke();
		}
	}
}

function drawTextOn(c: CanvasRenderingContext2D, t: TextLabel, isDark: boolean) {
	c.font = `${t.fontSize}px Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`;
	c.fillStyle = resolveColor(t.color, isDark);
	c.textBaseline = 'top';
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
	activeSnap: Point | null;
	activeSnapStart: Point | null;
}

export function render(
	opts: RenderOptions,
	offscreenRef: { canvas: HTMLCanvasElement | null; ctx: CanvasRenderingContext2D | null }
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

	if (!offscreenRef.canvas || offscreenRef.canvas.width !== w || offscreenRef.canvas.height !== h) {
		if (offscreenRef.canvas) {
			offscreenRef.canvas.width = 0;
			offscreenRef.canvas.height = 0;
		}
		offscreenRef.canvas = document.createElement('canvas');
		offscreenRef.canvas.width = w;
		offscreenRef.canvas.height = h;
		offscreenRef.ctx = offscreenRef.canvas.getContext('2d');
	}
	const offCtx = offscreenRef.ctx;
	if (!offCtx) {
		ctx.restore();
		return;
	}

	offCtx.clearRect(0, 0, w, h);
	offCtx.save();
	offCtx.scale(dpr, dpr);
	offCtx.scale(zoom, zoom);
	offCtx.translate(-camX, -camY);

	for (const s of opts.strokes) drawStrokeOn(offCtx, s, isDark);
	if (opts.currentStroke) drawStrokeOn(offCtx, opts.currentStroke, isDark);

	for (const s of opts.shapes) drawShapeOn(offCtx, s, isDark);
	if (opts.currentShape) drawShapeOn(offCtx, opts.currentShape, isDark);

	for (const t of opts.textLabels) drawTextOn(offCtx, t, isDark);
	offCtx.restore();

	ctx.drawImage(offscreenRef.canvas, 0, 0, w, h, 0, 0, w / dpr, h / dpr);

	if (opts.activeSnap || opts.activeSnapStart) {
		ctx.save();
		ctx.scale(zoom, zoom);
		ctx.translate(-camX, -camY);
		for (const snap of [opts.activeSnapStart, opts.activeSnap]) {
			if (snap) {
				ctx.beginPath();
				ctx.arc(snap.x, snap.y, 4 / zoom, 0, Math.PI * 2);
				ctx.fillStyle = 'rgba(59, 130, 246, 0.8)';
				ctx.fill();
				ctx.strokeStyle = '#ffffff';
				ctx.lineWidth = 1.5 / zoom;
				ctx.stroke();
			}
		}
		ctx.restore();
	}

	ctx.restore();
}
