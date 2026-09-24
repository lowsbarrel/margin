<script lang="ts">
	import { Eraser, Hand, Pencil, Type } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import type { ShapeKind, Tool } from '$lib/canvas/types';
	import { INK_COLOR, inkCss, isShapeTool } from '$lib/canvas/types';
	import CanvasShapePicker from './canvas/CanvasShapePicker.svelte';
	import CanvasStylePicker from './canvas/CanvasStylePicker.svelte';
	import * as Popover from '$lib/components/ui/popover/index.js';
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

	const POPOVER = 'w-auto gap-0 rounded-sm border border-border p-1.5 shadow-(--shadow-lg) ring-0';

	function pickShape(kind: ShapeKind) {
		onshape(kind);
		open = null;
	}
</script>

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

	<Popover.Root open={open === 'shape'} onOpenChange={(next) => (open = next ? 'shape' : null)}>
		<Popover.Trigger
			class={toolButtonClass(isShapeTool(tool))}
			title={shapeLabels[shape]()}
			aria-pressed={isShapeTool(tool)}
		>
			<ShapeIcon size={16} />
		</Popover.Trigger>
		<Popover.Content side="top" sideOffset={8} class={POPOVER}>
			<CanvasShapePicker {shape} onselect={pickShape} />
		</Popover.Content>
	</Popover.Root>

	<button
		class={toolButtonClass(tool === 'text')}
		onclick={() => ontool('text')}
		title={m.canvas_text()}
		aria-pressed={tool === 'text'}
	>
		<Type size={16} />
	</button>

	<span class="mx-1 h-5 w-px shrink-0 bg-border"></span>

	<Popover.Root open={open === 'style'} onOpenChange={(next) => (open = next ? 'style' : null)}>
		<Popover.Trigger
			class={colorButtonClass(open === 'style')}
			title={m.canvas_color()}
			aria-label={m.canvas_color()}
		>
			<span
				class="size-4.5 rounded-full shadow-[inset_0_0_0_1px_var(--color-border-strong)]"
				style:background={penColor === INK_COLOR ? inkCss : penColor}
			></span>
		</Popover.Trigger>
		<Popover.Content side="top" sideOffset={8} class={POPOVER}>
			<CanvasStylePicker
				{tool}
				color={penColor}
				{size}
				oncolor={(c) => (penColor = c)}
				onsize={onSizeChange}
			/>
		</Popover.Content>
	</Popover.Root>
</div>
