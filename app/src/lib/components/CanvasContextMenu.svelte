<script lang="ts">
	import * as m from '$lib/paraglide/messages.js';
	import type { Tool } from '$lib/canvas/types';
	import { colorPresets, sizePresets } from '$lib/canvas/types';

	interface Props {
		x: number;
		y: number;
		tool: Tool;
		penColor: string;
		currentSize: number;
		onSizeChange: (v: number) => void;
		onClearAll: () => void;
		onClose: () => void;
	}

	let {
		x,
		y,
		tool = $bindable(),
		penColor = $bindable(),
		currentSize,
		onSizeChange,
		onClearAll,
		onClose
	}: Props = $props();

	// Padding, radius, border and font-size are restated throughout this file
	// because app.css's `@layer base` rule styles every bare <button>; these
	// utilities sit in `@layer utilities` and so override it.
	const ITEM =
		'block w-full rounded-xs bg-transparent px-2.5 py-1.5 text-left text-sm [transition:background_var(--transition-fast),color_var(--transition-fast)]';

	// Active/hover are alternatives rather than stacked, mirroring the original
	// CSS where `.active` was declared after `:hover` and so won when an active
	// row was hovered.
	const itemCls = (active: boolean) =>
		`${ITEM} hover:bg-surface-3 hover:text-foreground ${
			active ? 'text-foreground' : 'text-muted-foreground'
		}`;

	// `box-content` keeps the 2px ring outside the 18px dot, as the original
	// `box-sizing: content-box` did. The transition names `scale` rather than
	// `transform` because Tailwind's `scale-*` sets the `scale` property.
	const SWATCH =
		'size-[18px] min-h-[18px] min-w-[18px] shrink-0 box-content rounded-full border-2 p-0 shadow-[inset_0_0_0_1px_var(--color-border-strong)] [transition:border-color_var(--transition-fast),scale_var(--transition-fast)]';

	const swatchCls = (active: boolean) =>
		`${SWATCH} ${active ? 'border-foreground scale-115' : 'border-transparent hover:scale-120'}`;

	const SIZE_BTN =
		'size-7 min-h-7 min-w-7 rounded-xs border bg-transparent p-0 text-xs [transition:background_var(--transition-fast),color_var(--transition-fast),border-color_var(--transition-fast)]';

	const sizeBtnCls = (active: boolean) =>
		`${SIZE_BTN} ${
			active
				? 'border-foreground bg-surface-2 text-foreground'
				: 'border-border text-muted-foreground hover:bg-surface-3'
		}`;
</script>

{#snippet sectionLabel(text: string)}
	<div class="px-2 pt-1 pb-0.5 text-xs tracking-[0.04em] text-subtle-foreground uppercase">
		{text}
	</div>
{/snippet}

{#snippet sep()}
	<div class="my-1 h-px bg-border"></div>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-[99]" onmousedown={onClose}></div>
<div
	class="fixed z-[100] min-w-[180px] rounded-sm border border-border bg-background p-1.5 shadow-[var(--shadow-lg)]"
	style:left={`${x}px`}
	style:top={`${y}px`}
	role="menu"
>
	{@render sectionLabel(m.canvas_tool())}
	<button
		class={itemCls(tool === 'pen')}
		onclick={() => {
			tool = 'pen';
			onClose();
		}}
		role="menuitem"
	>
		{m.canvas_pen()}
	</button>
	<button
		class={itemCls(tool === 'eraser')}
		onclick={() => {
			tool = 'eraser';
			onClose();
		}}
		role="menuitem"
	>
		{m.canvas_eraser()}
	</button>
	<button
		class={itemCls(tool === 'text')}
		onclick={() => {
			tool = 'text';
			onClose();
		}}
		role="menuitem"
	>
		{m.canvas_text()}
	</button>
	{@render sep()}

	{@render sectionLabel(m.canvas_color())}
	<div class="flex max-w-[160px] flex-wrap gap-1 px-2 py-1">
		{#each colorPresets as c (c)}
			<!-- `style:background` is canvas data (the pen colour), not theming. -->
			<button
				class={swatchCls(penColor === c)}
				style:background={c}
				onclick={() => {
					penColor = c;
					onClose();
				}}
				aria-label={c}
			></button>
		{/each}
	</div>
	{@render sep()}

	{@render sectionLabel(m.canvas_size())}
	<div class="flex flex-wrap gap-[3px] px-2 py-1">
		{#each sizePresets as s (s)}
			<button
				class={sizeBtnCls(currentSize === s)}
				onclick={() => {
					onSizeChange(s);
					onClose();
				}}
			>
				{s}
			</button>
		{/each}
	</div>
	{@render sep()}

	<!--
		The destructive row keeps its own colour on hover: in the original CSS
		`.ctx-item.danger` was declared after `.ctx-item:hover`, so hovering only
		changed the background. Hence no `hover:text-foreground` here.
	-->
	<button
		class="{ITEM} text-destructive hover:bg-destructive/10"
		onclick={onClearAll}
		role="menuitem"
	>
		{m.canvas_clear_all()}
	</button>
</div>
