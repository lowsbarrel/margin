<script lang="ts">
	import { onMount, onDestroy, tick } from 'svelte';
	import { readFileBytes } from '$lib/fs/bridge';
	import * as m from '$lib/paraglide/messages.js';
	import type { Point, Tool } from '$lib/canvas/types';
	import { deserialize } from '$lib/canvas/serialization';
	import { SnapCache } from '$lib/canvas/snapping';
	import CanvasToolbar from './CanvasToolbar.svelte';
	import CanvasContextMenu from './CanvasContextMenu.svelte';
	import { CanvasCamera } from './canvas/canvas-camera.svelte';
	import { createCanvasPersist } from './canvas/canvas-persist';
	import { CanvasScene } from './canvas/canvas-scene';
	import { createCanvasSurface } from './canvas/canvas-surface.svelte';
	import { ToolSizes } from './canvas/tool-sizes.svelte';

	interface Props {
		filePath: string;
		initialData?: string;
		onsave?: (content: string) => void;
	}

	let { filePath, initialData = '', onsave }: Props = $props();

	let canvasEl: HTMLCanvasElement;
	let wrapperEl: HTMLDivElement;

	const camera = new CanvasCamera();
	const scene = new CanvasScene();
	const sizes = new ToolSizes();
	const snapCache = new SnapCache();
	const persist = createCanvasPersist({
		path: () => filePath,
		scene,
		camera,
		onsave: () => onsave
	});
	const surface = createCanvasSurface({
		canvas: () => canvasEl,
		wrapper: () => wrapperEl,
		camera,
		scene,
		sizes,
		persist,
		snapCache,
		tool: () => tool,
		penColor: () => penColor,
		menuOpen: () => ctxMenu !== null,
		closeMenu: () => (ctxMenu = null),
		beginText
	});

	let tool = $state<Tool>('pen');
	let penColor = $state('#ffffff');
	let ctxMenu = $state<{ x: number; y: number } | null>(null);

	let editingText = $state<Point | null>(null);
	let textInputValue = $state('');
	let textInputEl = $state<HTMLInputElement>();
	let blurCommitBlocked = false;
	let showCursor = $state(false);

	let editingTextLocal = $derived(
		editingText ? camera.toScreen(editingText.x, editingText.y) : null
	);

	function loadDrawing(raw: string) {
		const parsed = deserialize(raw);
		if (!parsed) return;
		scene.load(parsed);
		camera.restore(parsed.camX, parsed.camY, parsed.zoom);
		snapCache.invalidate();
	}

	function beginText(pos: Point) {
		if (editingText) {
			blurCommitBlocked = true;
			commitText();
		}
		editingText = { x: pos.x, y: pos.y };
		textInputValue = '';
		blurCommitBlocked = true;
		tick().then(() => {
			textInputEl?.focus();
			blurCommitBlocked = false;
		});
	}

	function commitText() {
		const et = editingText;
		editingText = null;
		const val = textInputValue.trim();
		textInputValue = '';
		if (et && val) {
			scene.labels.push({ x: et.x, y: et.y, text: val, color: penColor, fontSize: sizes.text });
			snapCache.invalidate();
			surface.scheduleRender();
			persist.schedule();
		}
	}

	function handleTextBlur() {
		if (blurCommitBlocked) {
			blurCommitBlocked = false;
			return;
		}
		commitText();
	}

	function handleContextMenu(e: MouseEvent) {
		e.preventDefault();
		ctxMenu = { x: e.clientX, y: e.clientY };
	}

	function handleClearAll() {
		scene.clear();
		snapCache.invalidate();
		surface.scheduleRender();
		persist.schedule();
		ctxMenu = null;
	}

	function setZoom(newZoom: number) {
		camera.setZoom(newZoom);
		surface.scheduleRender();
		persist.schedule();
	}

	function resetView() {
		camera.reset();
		surface.scheduleRender();
		persist.schedule();
	}

	let resizeObserver: ResizeObserver;

	onMount(async () => {
		try {
			const bytes = await readFileBytes(filePath);
			loadDrawing(new TextDecoder().decode(bytes) || initialData);
		} catch {
			loadDrawing(initialData);
		}
		surface.resize();
		resizeObserver = new ResizeObserver(() => surface.resize());
		resizeObserver.observe(wrapperEl);
		window.addEventListener('margin:flush', persist.flush);
	});

	onDestroy(() => {
		resizeObserver?.disconnect();
		window.removeEventListener('margin:flush', persist.flush);
		surface.stop();
		persist.flush();
	});
</script>

<svelte:window onkeydown={surface.keyDown} onkeyup={surface.keyUp} onblur={persist.flush} />

<!-- One cursor class per state: Tailwind resolves same-layer utilities by stylesheet order, not class order. -->
<div
	class="relative h-full w-full overflow-hidden bg-background {tool === 'hand'
		? 'cursor-grab'
		: tool === 'text'
			? 'cursor-text'
			: 'cursor-crosshair'}"
	bind:this={wrapperEl}
	oncontextmenu={handleContextMenu}
	onpointerenter={() => (showCursor = true)}
	onpointerleave={() => (showCursor = false)}
	role="application"
	aria-label={m.canvas_label()}
>
	<canvas
		class="absolute top-0 left-0 block"
		bind:this={canvasEl}
		onpointerdown={surface.pointerDown}
		onpointermove={surface.pointerMove}
		onpointerup={surface.pointerUp}
		onwheel={surface.wheel}
	></canvas>

	<CanvasToolbar
		bind:tool
		bind:penColor
		currentSize={sizes.current(tool)}
		onSizeChange={(v) => sizes.set(tool, v)}
		zoom={camera.zoom}
		onZoomIn={() => setZoom(Math.min(5, camera.zoom * 1.2))}
		onZoomOut={() => setZoom(Math.max(0.1, camera.zoom / 1.2))}
		onResetView={resetView}
	/>

	{#if ctxMenu}
		<CanvasContextMenu
			x={ctxMenu.x}
			y={ctxMenu.y}
			bind:tool
			bind:penColor
			currentSize={sizes.current(tool)}
			onSizeChange={(v) => sizes.set(tool, v)}
			onClearAll={handleClearAll}
			onClose={() => (ctxMenu = null)}
		/>
	{/if}

	{#if editingText && editingTextLocal}
		<input
			class="absolute z-20 min-w-25 rounded-xs border border-dashed border-foreground bg-transparent px-1 py-0.5 font-sans caret-foreground outline-none"
			bind:this={textInputEl}
			style:left={`${editingTextLocal.x}px`}
			style:top={`${editingTextLocal.y}px`}
			style:font-size={`${sizes.text * camera.zoom}px`}
			style:color={penColor}
			bind:value={textInputValue}
			onkeydown={(e) => {
				if (e.key === 'Enter') commitText();
				if (e.key === 'Escape') {
					editingText = null;
					textInputValue = '';
				}
				e.stopPropagation();
			}}
			onblur={handleTextBlur}
		/>
	{/if}

	{#if showCursor && !camera.panning && !ctxMenu && tool !== 'text'}
		<div
			class="pointer-events-none fixed z-50 min-h-1 min-w-1 -translate-x-1/2 -translate-y-1/2 rounded-full border-[1.5px] [transition:width_var(--transition-fast),height_var(--transition-fast)]"
			style:left={`${surface.cursorX}px`}
			style:top={`${surface.cursorY}px`}
			style:width={`${sizes.current(tool) * camera.zoom}px`}
			style:height={`${sizes.current(tool) * camera.zoom}px`}
			style:border-color={tool === 'eraser' ? 'rgba(255,255,255,0.5)' : penColor}
		></div>
	{/if}
</div>
