<script lang="ts">
	import * as m from '$lib/paraglide/messages.js';
	import type { StrokeWidth, Tool } from '$lib/canvas/types';
	import { colorPresets, nearestWidth, widthOptions } from '$lib/canvas/types';
	import { swatchClass, toolButtonClass } from './control-classes';

	interface Props {
		tool: Tool;
		color: string;
		size: number;
		oncolor: (color: string) => void;
		onsize: (size: number) => void;
	}

	let { tool, color, size, oncolor, onsize }: Props = $props();

	const WIDTH_LABELS: Record<StrokeWidth, () => string> = {
		thin: m.canvas_width_thin,
		medium: m.canvas_width_medium,
		thick: m.canvas_width_thick
	};

	const WIDTH_BARS: Record<StrokeWidth, number> = { thin: 2, medium: 4, thick: 6 };

	const options = $derived(widthOptions(tool));
	const active = $derived(nearestWidth(tool, size));
</script>

<div class="flex items-center gap-1 px-1 py-0.5">
	{#each colorPresets as c (c)}
		<button
			class={swatchClass(color === c)}
			style:background={c}
			onclick={() => oncolor(c)}
			title={c}
			aria-label={c}
		></button>
	{/each}
</div>

<div class="my-1 h-px bg-border"></div>

<div class="flex items-center gap-0.5 p-0.5">
	{#each options as option (option.width)}
		<button
			class={toolButtonClass(active === option.width)}
			onclick={() => onsize(option.value)}
			title={WIDTH_LABELS[option.width]()}
			aria-pressed={active === option.width}
		>
			<span
				class="block w-4.5 rounded-full bg-current"
				style:height={`${WIDTH_BARS[option.width]}px`}
			></span>
		</button>
	{/each}
</div>
