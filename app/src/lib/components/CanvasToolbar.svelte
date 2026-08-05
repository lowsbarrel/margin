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

	// `p-0` / `rounded-xs` are restated rather than inherited: app.css's
	// `@layer base` button rule gives every bare <button> 8px/14px padding and an
	// 8px radius, which would inflate these 30px icon buttons.
	const TOOL_BTN =
		'flex size-[30px] min-h-[30px] min-w-[30px] shrink-0 items-center justify-center rounded-xs bg-transparent p-0 [transition:background_var(--transition-fast),color_var(--transition-fast)]';

	// Active and hover states are emitted as alternatives rather than stacked,
	// mirroring the original CSS where `.active` was declared after `:hover` and
	// therefore won on an active button being hovered.
	const toolBtnCls = (active: boolean) =>
		`${TOOL_BTN} ${
			active
				? 'bg-surface-2 text-foreground'
				: 'text-subtle-foreground hover:bg-surface-3 hover:text-foreground'
		}`;

	// `box-content` keeps the 2px ring outside the 18px dot, as the original
	// `box-sizing: content-box` did. The transition names `scale` rather than
	// `transform` because Tailwind's `scale-*` sets the `scale` property.
	const SWATCH =
		'size-[18px] min-h-[18px] min-w-[18px] shrink-0 box-content rounded-full border-2 p-0 shadow-[inset_0_0_0_1px_var(--color-border-strong)] [transition:border-color_var(--transition-fast),scale_var(--transition-fast)]';

	const swatchCls = (active: boolean) =>
		`${SWATCH} ${active ? 'border-foreground scale-115' : 'border-transparent hover:scale-120'}`;
</script>

{#snippet sep()}
	<span class="mx-1 h-5 w-px shrink-0 bg-border"></span>
{/snippet}

<div
	class="absolute bottom-4 left-1/2 z-10 flex max-w-[calc(100%-32px)] -translate-x-1/2 flex-wrap items-center justify-center gap-1.5 rounded-sm border border-border bg-background px-2.5 py-1.5 shadow-[var(--shadow-lg)] select-none"
>
	<div class="flex shrink-0 items-center gap-0.5">
		<button
			class={toolBtnCls(tool === 'hand')}
			onclick={() => (tool = 'hand')}
			title={m.canvas_hand()}
		>
			<Hand size={16} />
		</button>
		<button
			class={toolBtnCls(tool === 'pen')}
			onclick={() => (tool = 'pen')}
			title={m.canvas_pen()}
		>
			<Pencil size={16} />
		</button>
		<button
			class={toolBtnCls(tool === 'eraser')}
			onclick={() => (tool = 'eraser')}
			title={m.canvas_eraser()}
		>
			<Eraser size={16} />
		</button>
		{@render sep()}
		<button
			class={toolBtnCls(tool === 'rect')}
			onclick={() => (tool = 'rect')}
			title={m.canvas_rect()}
		>
			<Square size={16} />
		</button>
		<button
			class={toolBtnCls(tool === 'ellipse')}
			onclick={() => (tool = 'ellipse')}
			title={m.canvas_ellipse()}
		>
			<Circle size={16} />
		</button>
		<button
			class={toolBtnCls(tool === 'line')}
			onclick={() => (tool = 'line')}
			title={m.canvas_line()}
		>
			<Minus size={16} />
		</button>
		<button
			class={toolBtnCls(tool === 'arrow')}
			onclick={() => (tool = 'arrow')}
			title={m.canvas_arrow()}
		>
			<ArrowUpRight size={16} />
		</button>
		<button
			class={toolBtnCls(tool === 'text')}
			onclick={() => (tool = 'text')}
			title={m.canvas_text()}
		>
			<Type size={16} />
		</button>
	</div>

	{@render sep()}

	<div class="flex shrink-0 items-center gap-[3px]">
		{#each colorPresets as c (c)}
			<!-- `style:background` is canvas data (the pen colour), not theming. -->
			<button
				class={swatchCls(penColor === c)}
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
			class="w-20 min-w-[60px] cursor-pointer accent-foreground"
		/>
	</div>

	{@render sep()}

	<div class="flex shrink-0 items-center gap-0.5">
		<button class={toolBtnCls(false)} onclick={onZoomIn} title={m.canvas_zoom_in()}>
			<ZoomIn size={14} />
		</button>
		<span class="min-w-9 shrink-0 text-center text-xs text-subtle-foreground"
			>{Math.round(zoom * 100)}%</span
		>
		<button class={toolBtnCls(false)} onclick={onZoomOut} title={m.canvas_zoom_out()}>
			<ZoomOut size={14} />
		</button>
		<button class={toolBtnCls(false)} onclick={onResetView} title={m.canvas_reset_view()}>
			<RotateCcw size={14} />
		</button>
	</div>
</div>
