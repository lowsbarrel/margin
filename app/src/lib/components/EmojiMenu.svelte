<script lang="ts">
	import type { EmojiItem } from '$lib/editor/emoji-command';

	interface Props {
		items: EmojiItem[];
		selectedIndex: number;
		onselect: (item: EmojiItem) => void;
		onhover: (index: number) => void;
	}

	let { items, selectedIndex, onselect, onhover }: Props = $props();

	let listEl: HTMLDivElement;

	export function scrollToSelected() {
		if (!listEl) return;
		const el = listEl.children[selectedIndex] as HTMLElement | undefined;
		if (!el) return;
		const top = el.offsetTop;
		const bottom = top + el.offsetHeight;
		if (top < listEl.scrollTop) {
			listEl.scrollTop = top;
		} else if (bottom > listEl.scrollTop + listEl.clientHeight) {
			listEl.scrollTop = bottom - listEl.clientHeight;
		}
	}
</script>

<!-- `.surface-popover` (shared, in `$lib/styles/components.css`) carries the solid
     fill, hairline border and the shadow every floating menu gets. The `!`
     modifiers below counter `src/app.css`'s unlayered bare-`button` rule, which
     otherwise outranks the padding/radius utilities. -->
<div class="surface-popover max-h-60 min-w-55 overflow-y-auto p-1" bind:this={listEl}>
	{#if items.length === 0}
		<div class="px-3 py-2 text-sm text-muted-foreground">No results</div>
	{:else}
		{#each items as item, index (item.emoji + item.name)}
			<button
				class="flex w-full items-center gap-2 rounded-xs px-2 py-1 text-left text-sm text-foreground transition-colors hover:bg-surface-3 [&.is-selected]:bg-surface-3"
				class:is-selected={index === selectedIndex}
				onmouseenter={() => onhover(index)}
				onclick={() => onselect(item)}
			>
				<span class="w-6 text-center text-xl">{item.emoji}</span>
				<span class="min-w-0 flex-1 truncate text-sm text-muted-foreground">{item.name}</span>
			</button>
		{/each}
	{/if}
</div>
