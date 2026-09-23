<script lang="ts">
	import {
		Pencil,
		Eraser,
		Square,
		Circle,
		Minus,
		ArrowUpRight,
		ZoomIn,
		ZoomOut,
		RotateCcw,
		Hand,
		Type
	} from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import type { Tool } from '$lib/canvas/types';
	import { colorPresets } from '$lib/canvas/types';
	import { swatchClass, toolButtonClass } from './canvas/control-classes';

	interface Props {
		tool: Tool;
		penColor: string;
		currentSize: number;
		onSizeChange: (v: number) => void;
		zoom: number;
		onZoomIn: () => void;
		onZoomOut: () => void;
		onResetView: () => void;
	}

	let {
		tool = $bindable(),
		penColor = $bindable(),
		currentSize,
		onSizeChange,
		zoom,
		onZoomIn,
		onZoomOut,
		onResetView
	}: Props = $props();
</script>

{#snippet sep()}
	<span class="mx-1 h-5 w-px shrink-0 bg-border"></span>
{/snippet}

<div
	class="absolute bottom-4 left-1/2 z-10 flex max-w-[calc(100%-32px)] -translate-x-1/2 flex-wrap items-center justify-center gap-1.5 rounded-sm border border-border bg-background px-2.5 py-1.5 shadow-(--shadow-lg) select-none"
>
	<div class="flex shrink-0 items-center gap-0.5">
		<button
			class={toolButtonClass(tool === 'hand')}
			onclick={() => (tool = 'hand')}
			title={m.canvas_hand()}
		>
			<Hand size={16} />
		</button>
		<button
			class={toolButtonClass(tool === 'pen')}
			onclick={() => (tool = 'pen')}
			title={m.canvas_pen()}
		>
			<Pencil size={16} />
		</button>
		<button
			class={toolButtonClass(tool === 'eraser')}
			onclick={() => (tool = 'eraser')}
			title={m.canvas_eraser()}
		>
			<Eraser size={16} />
		</button>
		{@render sep()}
		<button
			class={toolButtonClass(tool === 'rect')}
			onclick={() => (tool = 'rect')}
			title={m.canvas_rect()}
		>
			<Square size={16} />
		</button>
		<button
			class={toolButtonClass(tool === 'ellipse')}
			onclick={() => (tool = 'ellipse')}
			title={m.canvas_ellipse()}
		>
			<Circle size={16} />
		</button>
		<button
			class={toolButtonClass(tool === 'line')}
			onclick={() => (tool = 'line')}
			title={m.canvas_line()}
		>
			<Minus size={16} />
		</button>
		<button
			class={toolButtonClass(tool === 'arrow')}
			onclick={() => (tool = 'arrow')}
			title={m.canvas_arrow()}
		>
			<ArrowUpRight size={16} />
		</button>
		<button
			class={toolButtonClass(tool === 'text')}
			onclick={() => (tool = 'text')}
			title={m.canvas_text()}
		>
			<Type size={16} />
		</button>
	</div>

	{@render sep()}

	<div class="flex shrink-0 items-center gap-0.75">
		{#each colorPresets as c (c)}
			<button
				class={swatchClass(penColor === c)}
				style:background={c}
				onclick={() => (penColor = c)}
				title={c}
			></button>
		{/each}
	</div>

	{@render sep()}

	<div class="flex shrink-0 items-center gap-1.5">
		<label
			class="min-w-8 shrink-0 text-right text-xs text-subtle-foreground"
			for="canvas-size-slider">{currentSize}px</label
		>
		<input
			id="canvas-size-slider"
			type="range"
			min={tool === 'text' ? 8 : 1}
			max={tool === 'eraser' ? 60 : tool === 'text' ? 72 : 30}
			value={currentSize}
			oninput={(e) => onSizeChange(Number(e.currentTarget.value))}
			class="w-20 min-w-15 cursor-pointer accent-foreground"
		/>
	</div>

	{@render sep()}

	<div class="flex shrink-0 items-center gap-0.5">
		<button class={toolButtonClass(false)} onclick={onZoomIn} title={m.canvas_zoom_in()}>
			<ZoomIn size={14} />
		</button>
		<span class="min-w-9 shrink-0 text-center text-xs text-subtle-foreground"
			>{Math.round(zoom * 100)}%</span
		>
		<button class={toolButtonClass(false)} onclick={onZoomOut} title={m.canvas_zoom_out()}>
			<ZoomOut size={14} />
		</button>
		<button class={toolButtonClass(false)} onclick={onResetView} title={m.canvas_reset_view()}>
			<RotateCcw size={14} />
		</button>
	</div>
</div>
