<script lang="ts">
	import * as m from '$lib/paraglide/messages.js';
	import type { Tool } from '$lib/canvas/types';
	import { colorPresets, sizePresets } from '$lib/canvas/types';
	import { MENU_ITEM, menuItemClass, sizeButtonClass, swatchClass } from './canvas/control-classes';

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
<div class="fixed inset-0 z-99" onmousedown={onClose}></div>
<div
	class="fixed z-100 min-w-45 rounded-sm border border-border bg-background p-1.5 shadow-(--shadow-lg)"
	style:left={`${x}px`}
	style:top={`${y}px`}
	role="menu"
>
	{@render sectionLabel(m.canvas_tool())}
	<button
		class={menuItemClass(tool === 'pen')}
		onclick={() => {
			tool = 'pen';
			onClose();
		}}
		role="menuitem"
	>
		{m.canvas_pen()}
	</button>
	<button
		class={menuItemClass(tool === 'eraser')}
		onclick={() => {
			tool = 'eraser';
			onClose();
		}}
		role="menuitem"
	>
		{m.canvas_eraser()}
	</button>
	<button
		class={menuItemClass(tool === 'text')}
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
	<div class="flex max-w-40 flex-wrap gap-1 px-2 py-1">
		{#each colorPresets as c (c)}
			<button
				class={swatchClass(penColor === c)}
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
	<div class="flex flex-wrap gap-0.75 px-2 py-1">
		{#each sizePresets as s (s)}
			<button
				class={sizeButtonClass(currentSize === s)}
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

	<button
		class="{MENU_ITEM} text-destructive hover:bg-destructive/10"
		onclick={onClearAll}
		role="menuitem"
	>
		{m.canvas_clear_all()}
	</button>
</div>
