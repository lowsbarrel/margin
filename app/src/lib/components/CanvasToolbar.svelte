<script lang="ts">
	import { Eraser, Hand, Pencil, Type } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import type { ShapeKind, Tool } from '$lib/canvas/types';
	import { isShapeTool } from '$lib/canvas/types';
	import CanvasPopover from './canvas/CanvasPopover.svelte';
	import CanvasShapePicker from './canvas/CanvasShapePicker.svelte';
	import CanvasStylePicker from './canvas/CanvasStylePicker.svelte';
	import { colorButtonClass, toolButtonClass } from './canvas/control-classes';
	import { shapeIcons, shapeLabels } from './canvas/shape-ui';

	interface Props {
		tool: Tool;
		shape: ShapeKind;
		penColor: string;
		size: number;
		ontool: (tool: Tool) => void;
		onshape: (shape: ShapeKind) => void;
		onSizeChange: (size: number) => void;
	}

	let {
		tool,
		shape,
		penColor = $bindable(),
		size,
		ontool,
		onshape,
		onSizeChange
	}: Props = $props();

	let open = $state<'shape' | 'style' | null>(null);

	const ShapeIcon = $derived(shapeIcons[shape]);

	function toggle(which: 'shape' | 'style') {
		open = open === which ? null : which;
	}

	function pickShape(kind: ShapeKind) {
		onshape(kind);
		open = null;
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (open = null)} />

<div
	class="absolute bottom-4 left-1/2 z-10 flex -translate-x-1/2 items-center gap-0.5 rounded-full border border-border bg-background px-1.5 py-1 shadow-(--shadow-lg) select-none"
>
	<button
		class={toolButtonClass(tool === 'hand')}
		onclick={() => ontool('hand')}
		title={m.canvas_hand()}
		aria-pressed={tool === 'hand'}
	>
		<Hand size={16} />
	</button>
	<button
		class={toolButtonClass(tool === 'pen')}
		onclick={() => ontool('pen')}
		title={m.canvas_pen()}
		aria-pressed={tool === 'pen'}
	>
		<Pencil size={16} />
	</button>
	<button
		class={toolButtonClass(tool === 'eraser')}
		onclick={() => ontool('eraser')}
		title={m.canvas_eraser()}
		aria-pressed={tool === 'eraser'}
	>
		<Eraser size={16} />
	</button>

	<div class="relative">
		<button
			class={toolButtonClass(isShapeTool(tool))}
			onclick={() => toggle('shape')}
			title={shapeLabels[shape]()}
			aria-expanded={open === 'shape'}
			aria-pressed={isShapeTool(tool)}
		>
			<ShapeIcon size={16} />
		</button>
		{#if open === 'shape'}
			<CanvasPopover onclose={() => (open = null)}>
				<CanvasShapePicker {shape} onselect={pickShape} />
			</CanvasPopover>
		{/if}
	</div>

	<button
		class={toolButtonClass(tool === 'text')}
		onclick={() => ontool('text')}
		title={m.canvas_text()}
		aria-pressed={tool === 'text'}
	>
		<Type size={16} />
	</button>

	<span class="mx-1 h-5 w-px shrink-0 bg-border"></span>

	<div class="relative">
		<button
			class={colorButtonClass(open === 'style')}
			onclick={() => toggle('style')}
			title={m.canvas_color()}
			aria-label={m.canvas_color()}
			aria-expanded={open === 'style'}
		>
			<span
				class="size-4.5 rounded-full shadow-[inset_0_0_0_1px_var(--color-border-strong)]"
				style:background={penColor}
			></span>
		</button>
		{#if open === 'style'}
			<CanvasPopover onclose={() => (open = null)}>
				<CanvasStylePicker
					{tool}
					color={penColor}
					{size}
					oncolor={(c) => (penColor = c)}
					onsize={onSizeChange}
				/>
			</CanvasPopover>
		{/if}
	</div>
</div>
