<script lang="ts">
	import { tick } from 'svelte';
	import { ZoomIn, ZoomOut, Expand } from '@lucide/svelte';
	import { formatBytes } from '$lib/utils/bytes';
	import { fileNameFromSrc } from '$lib/utils/mime';
	import * as m from '$lib/paraglide/messages.js';

	// Padding and radius are restated because app.css's base button rule gives
	// every bare <button> its own box; these utilities sit in `@layer utilities`
	// and so override it.
	const CONTROL =
		'flex h-6.5 min-w-6.5 items-center justify-center rounded-xs bg-transparent px-1.5 text-sm text-subtle-foreground transition-colors hover:bg-surface-3 hover:text-foreground';

	interface Props {
		src: string;
		/** File name for the caption; derived from `src` when the caller has none. */
		name?: string;
		/** Bytes on disk, when the caller knows them. */
		size?: number;
		/** `lightbox` overlays its chrome on the (always dark) lightbox scrim. */
		variant?: 'tab' | 'lightbox';
		/** Called for a click on the area around the image, never for a pan. */
		onbackgroundclick?: () => void;
	}

	let { src, name, size, variant = 'tab', onbackgroundclick }: Props = $props();

	const MIN_ZOOM = 0.1;
	const MAX_ZOOM = 8;
	const ZOOM_STEP = 1.25;

	let viewportEl = $state<HTMLDivElement>(undefined!);
	let canvasEl = $state<HTMLDivElement>(undefined!);
	let viewportW = $state(0);
	let viewportH = $state(0);
	let natural = $state<{ w: number; h: number } | null>(null);
	let failed = $state(false);
	/** Scale to fall back to once the user zooms away from fit. */
	let zoom = $state(1);
	let fitMode = $state(true);
	let panning = $state(false);
	let panFrom: { x: number; y: number; left: number; top: number } | null = null;
	let dragged = false;

	let isLightbox = $derived(variant === 'lightbox');

	let displayName = $derived(name ?? fileNameFromSrc(src));
	// Fit never upscales: blowing a 16px icon up to fill the pane is blur, not help.
	let fitted = $derived(
		natural && viewportW > 0 && viewportH > 0
			? Math.min(1, viewportW / natural.w, viewportH / natural.h)
			: 1
	);
	let scale = $derived(fitMode ? fitted : zoom);
	let pixelWidth = $derived(natural ? Math.max(1, Math.round(natural.w * scale)) : 0);
	let pixelHeight = $derived(natural ? Math.max(1, Math.round(natural.h * scale)) : 0);
	let meta = $derived(
		[natural ? `${natural.w} × ${natural.h}` : null, size === undefined ? null : formatBytes(size)]
			.filter((part) => part !== null)
			.join(' · ')
	);

	$effect(() => {
		const el = viewportEl;
		if (!el) return;
		// contentRect excludes the scrollbar, so "fit" means fit the visible area.
		const observer = new ResizeObserver(([entry]) => {
			viewportW = entry.contentRect.width;
			viewportH = entry.contentRect.height;
		});
		observer.observe(el);
		return () => observer.disconnect();
	});

	/**
	 * Rescales around a point in viewport coordinates, by keeping the image
	 * pixel under that point where it is: scroll is adjusted by the same delta
	 * the re-laid-out box moved, which is why the size is set as width/height
	 * rather than a transform — a transform leaves overflow unscrollable.
	 */
	async function zoomTo(next: number, clientX?: number, clientY?: number) {
		const clamped = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, next));
		const previous = scale;
		if (clientX === undefined || clientY === undefined) {
			fitMode = false;
			zoom = clamped;
			return;
		}
		const before = canvasEl.getBoundingClientRect();
		const offsetX = (clientX - before.left) / previous;
		const offsetY = (clientY - before.top) / previous;
		fitMode = false;
		zoom = clamped;
		await tick();
		const after = canvasEl.getBoundingClientRect();
		viewportEl.scrollLeft += after.left + offsetX * clamped - clientX;
		viewportEl.scrollTop += after.top + offsetY * clamped - clientY;
	}

	function zoomAtCenter(factor: number) {
		const rect = viewportEl.getBoundingClientRect();
		void zoomTo(scale * factor, rect.left + rect.width / 2, rect.top + rect.height / 2);
	}

	function handleWheel(e: WheelEvent) {
		// Plain wheel belongs to the scroller: past 100% that is how the far edge is reached.
		if (!e.ctrlKey && !e.metaKey) return;
		e.preventDefault();
		const factor = Math.min(2, Math.max(0.5, Math.exp(-e.deltaY * 0.005)));
		void zoomTo(scale * factor, e.clientX, e.clientY);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.ctrlKey || e.metaKey || e.altKey) return;
		if (e.key === '+' || e.key === '=') zoomAtCenter(ZOOM_STEP);
		else if (e.key === '-' || e.key === '_') zoomAtCenter(1 / ZOOM_STEP);
		else if (e.key === '0') {
			fitMode = false;
			zoom = 1;
		} else if (e.key === 'f') fitMode = true;
		else return;
		e.preventDefault();
	}

	/** Natural size is what every zoom is relative to, so it is read on load. */
	function handleLoad(e: Event) {
		const img = e.currentTarget as HTMLImageElement;
		natural = { w: img.naturalWidth, h: img.naturalHeight };
		failed = false;
	}

	function startPan(e: PointerEvent) {
		if (e.button !== 0 || !natural) return;
		dragged = false;
		panFrom = {
			x: e.clientX,
			y: e.clientY,
			left: viewportEl.scrollLeft,
			top: viewportEl.scrollTop
		};
		window.addEventListener('pointermove', movePan);
		window.addEventListener('pointerup', endPan);
		window.addEventListener('pointercancel', endPan);
	}

	function movePan(e: PointerEvent) {
		if (!panFrom) return;
		const dx = e.clientX - panFrom.x;
		const dy = e.clientY - panFrom.y;
		if (!dragged) {
			if (Math.abs(dx) + Math.abs(dy) <= 3) return;
			dragged = true;
			panning = true;
		}
		e.preventDefault();
		viewportEl.scrollLeft = panFrom.left - dx;
		viewportEl.scrollTop = panFrom.top - dy;
	}

	function endPan() {
		panFrom = null;
		panning = false;
		window.removeEventListener('pointermove', movePan);
		window.removeEventListener('pointerup', endPan);
		window.removeEventListener('pointercancel', endPan);
	}

	function handleClick(e: MouseEvent) {
		if (!isLightbox || dragged || !onbackgroundclick) return;
		if (canvasEl?.contains(e.target as Node)) return;
		onbackgroundclick();
	}
</script>

<!-- Zoom keys sit on the wrapper so they keep working while focus is on one of
     the toolbar buttons; the viewport is the scroll surface those keys zoom. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="relative flex h-full w-full flex-col overflow-hidden {isLightbox ? '' : 'bg-surface-2'}"
	onkeydown={handleKeydown}
>
	<!-- Focusable so the zoom keys have a target, pannable by pointer, and in the
	     lightbox closable from the background — which Escape already does for
	     anyone not using a mouse. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		class="relative flex min-h-0 flex-1 overflow-auto {panning ? 'cursor-grabbing' : 'cursor-grab'}"
		bind:this={viewportEl}
		tabindex={0}
		role="application"
		aria-label={displayName}
		onwheel={handleWheel}
		onpointerdown={startPan}
		onclick={handleClick}
	>
		{#if failed}
			<div class="m-auto max-w-80 px-6 text-center text-[13px] text-subtle-foreground">
				{m.viewer_image_failed()}
			</div>
		{:else}
			<div
				class="image-canvas m-auto flex-none overflow-hidden select-none {isLightbox
					? 'shadow-(--shadow-lg)'
					: 'shadow-(--shadow-md)'}"
				bind:this={canvasEl}
				style:width="{pixelWidth}px"
				style:height="{pixelHeight}px"
			>
				<img
					{src}
					alt={displayName}
					draggable="false"
					onload={handleLoad}
					onerror={() => (failed = true)}
					class="pointer-events-none block h-full w-full"
				/>
			</div>
		{/if}
	</div>

	<div
		class="flex items-center gap-1 {isLightbox
			? 'pointer-events-none absolute inset-x-0 bottom-0 justify-center bg-black/45 py-1.5'
			: 'border-t border-border bg-surface-1 p-1.5'}"
	>
		{#if !isLightbox}
			<span class="flex min-w-0 items-center gap-2 px-2 text-xs whitespace-nowrap">
				<span class="min-w-0 truncate text-foreground">{displayName}</span>
				{#if meta}
					<span class="shrink-0 text-subtle-foreground tabular-nums">{meta}</span>
				{/if}
			</span>
			<span class="flex-1"></span>
		{/if}

		<div
			class={isLightbox
				? 'pointer-events-auto flex items-center gap-1 rounded-sm bg-black/30 px-1'
				: 'contents'}
		>
			<button
				class={CONTROL}
				onclick={() => zoomAtCenter(1 / ZOOM_STEP)}
				title={m.viewer_zoom_out()}
				aria-label={m.viewer_zoom_out()}
			>
				<ZoomOut size={14} />
			</button>
			<button
				class={CONTROL}
				onclick={() => zoomAtCenter(ZOOM_STEP)}
				title={m.viewer_zoom_in()}
				aria-label={m.viewer_zoom_in()}
			>
				<ZoomIn size={14} />
			</button>
			<span class="min-w-12 text-center text-sm tabular-nums">{Math.round(scale * 100)}%</span>
			<button
				class={CONTROL}
				onclick={() => (fitMode = true)}
				title={m.viewer_fit()}
				aria-label={m.viewer_fit()}
				aria-pressed={fitMode}
			>
				<Expand size={14} />
			</button>
			<button
				class={CONTROL}
				onclick={() => {
					fitMode = false;
					zoom = 1;
				}}
				title={m.viewer_actual_size()}
				aria-label={m.viewer_actual_size()}
			>
				100%
			</button>
		</div>
	</div>
</div>

<style>
	/* Transparency checkerboard, two token surfaces only. */
	.image-canvas {
		background-color: var(--color-bg-primary);
		background-image:
			linear-gradient(45deg, var(--color-surface-2) 25%, transparent 25%),
			linear-gradient(-45deg, var(--color-surface-2) 25%, transparent 25%),
			linear-gradient(45deg, transparent 75%, var(--color-surface-2) 75%),
			linear-gradient(-45deg, transparent 75%, var(--color-surface-2) 75%);
		background-size: 16px 16px;
		background-position:
			0 0,
			0 8px,
			8px -8px,
			-8px 0;
	}
</style>
