<script lang="ts">
	// Padding and radius are restated because app.css's `@layer base` button rule
	// gives every bare <button> 8px/14px padding and an 8px radius; these
	// utilities sit in `@layer utilities` and so override it.
	const ZOOM_BTN =
		'rounded-xs bg-transparent px-2 py-0.5 text-sm text-subtle-foreground hover:bg-surface-3 hover:text-foreground';

	interface Props {
		src: string;
		alt: string;
	}

	let { src, alt }: Props = $props();
	let scale = $state(1);
	const ZOOM_STEP = 0.15;
	const MIN_SCALE = 0.1;
	const MAX_SCALE = 5;

	function handleWheel(e: WheelEvent) {
		if (!e.ctrlKey && !e.metaKey) return;
		e.preventDefault();
		const delta = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
		scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale + delta));
	}

	function resetZoom() {
		scale = 1;
	}
</script>

<div class="flex h-full w-full flex-col overflow-hidden" onwheel={handleWheel}>
	<div class="flex flex-1 items-center justify-center overflow-auto p-6">
		<img
			{src}
			{alt}
			style:transform="scale({scale})"
			draggable="false"
			class="max-h-full max-w-full rounded-xs object-contain select-none [transition:transform_var(--transition-fast)]"
		/>
	</div>
	<div class="flex items-center justify-center gap-0.5 border-t border-border bg-surface-1 p-1.5">
		<button class={ZOOM_BTN} onclick={() => (scale = Math.max(MIN_SCALE, scale - ZOOM_STEP))}
			>−</button
		>
		<button class="{ZOOM_BTN} min-w-12 text-center tabular-nums" onclick={resetZoom}
			>{Math.round(scale * 100)}%</button
		>
		<button class={ZOOM_BTN} onclick={() => (scale = Math.min(MAX_SCALE, scale + ZOOM_STEP))}
			>+</button
		>
	</div>
</div>
