<script lang="ts">
	import * as m from '$lib/paraglide/messages.js';
	import { MENU_ITEM } from './canvas/control-classes';

	interface Props {
		x: number;
		y: number;
		onClear: () => void;
		onClose: () => void;
	}

	let { x, y, onClear, onClose }: Props = $props();

	let confirming = $state(false);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-99" onmousedown={onClose}></div>
<div
	class="fixed z-100 w-52 rounded-sm border border-border bg-background p-1.5 shadow-(--shadow-lg)"
	style:left={`${x}px`}
	style:top={`${y}px`}
>
	{#if confirming}
		<p class="px-2.5 py-1.5 text-sm text-muted-foreground">{m.canvas_clear_confirm()}</p>
		<div class="flex gap-1.5 px-0.5 pt-0.5">
			<button
				class="{MENU_ITEM} border border-border text-center text-muted-foreground hover:bg-surface-3 hover:text-foreground"
				onclick={onClose}>{m.canvas_clear_no()}</button
			>
			<button
				class="{MENU_ITEM} border border-destructive/40 text-center text-destructive hover:bg-destructive/10"
				onclick={onClear}>{m.canvas_clear_yes()}</button
			>
		</div>
	{:else}
		<button
			class="{MENU_ITEM} text-destructive hover:bg-destructive/10"
			onclick={() => (confirming = true)}>{m.canvas_clear_all()}</button
		>
	{/if}
</div>
