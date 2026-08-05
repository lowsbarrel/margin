<script lang="ts">
	import { onMount, tick, untrack } from 'svelte';

	export interface ContextMenuItem {
		label: string;
		onclick: () => void | Promise<void>;
		destructive?: boolean;
		disabled?: boolean;
	}

	interface Props {
		x: number;
		y: number;
		items: ContextMenuItem[];
		onclose: () => void;
	}

	let { x, y, items, onclose }: Props = $props();
	let menuEl: HTMLDivElement;
	let left = $state(untrack(() => x));
	let top = $state(untrack(() => y));

	// The anchor is passed in rather than read off `x`/`y` inside: the clamping
	// happens after `await tick()`, and reads past an await are not tracked by
	// the effect below. Taking them as arguments makes the dependency real.
	async function positionMenu(anchorX: number, anchorY: number) {
		await tick();
		if (!menuEl) return;
		const rect = menuEl.getBoundingClientRect();
		left = Math.max(8, Math.min(anchorX, window.innerWidth - rect.width - 8));
		top = Math.max(8, Math.min(anchorY, window.innerHeight - rect.height - 8));
	}

	$effect(() => {
		positionMenu(x, y);
	});

	onMount(() => {
		positionMenu(x, y);
		requestAnimationFrame(() => menuEl?.focus());
	});

	function handleDocumentMouseDown(event: MouseEvent) {
		if (!menuEl?.contains(event.target as Node)) {
			onclose();
		}
	}

	function handleDocumentContextMenu(event: MouseEvent) {
		if (!menuEl?.contains(event.target as Node)) {
			onclose();
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			onclose();
		}
	}

	async function runItem(item: ContextMenuItem) {
		if (item.disabled) return;
		onclose();
		await item.onclick();
	}
</script>

<svelte:document
	onmousedown={handleDocumentMouseDown}
	oncontextmenu={handleDocumentContextMenu}
	onkeydown={handleKeydown}
/>

<!-- Surface (solid fill, hairline border, shadow) comes from the shared
     `.surface-popover` class; the rest is utilities. The `!` modifiers counter
     `src/app.css`'s unlayered bare-`button` rule, which outranks utilities on
     the properties it sets (padding, weight, disabled cursor). -->
<div
	class="surface-popover fixed z-[200] min-w-44 p-1 outline-none"
	bind:this={menuEl}
	style:left={`${left}px`}
	style:top={`${top}px`}
	tabindex={-1}
	role="menu"
>
	{#each items as item (item.label)}
		<button
			class="block w-full rounded-sm px-2.5 py-[7px] text-left text-sm font-normal tracking-normal transition-colors disabled:cursor-default disabled:opacity-40 {item.destructive
				? 'text-destructive enabled:hover:bg-destructive/10'
				: 'text-muted-foreground enabled:hover:bg-surface-1 enabled:hover:text-foreground'}"
			disabled={item.disabled}
			onclick={() => runItem(item)}
			role="menuitem"
		>
			{item.label}
		</button>
	{/each}
</div>
