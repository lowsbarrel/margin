<script lang="ts">
	import type { MentionItem } from '$lib/editor/mention-command';
	import { FileText } from '@lucide/svelte';

	interface Props {
		items: MentionItem[];
		selectedIndex: number;
		onselect: (item: MentionItem) => void;
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

<!-- Surface comes from the shared `.surface-popover` class; the `!` modifiers
     counter `src/app.css`'s unlayered bare-`button` rule. -->
<div class="surface-popover max-h-70 min-w-55 overflow-y-auto p-1" bind:this={listEl}>
	{#if items.length === 0}
		<div class="px-3 py-2 text-sm text-subtle-foreground">No matching documents</div>
	{:else}
		{#each items as item, index (item.path)}
			<button
				class="flex w-full items-center gap-2 rounded-sm px-2.5 py-1.5 text-left text-sm text-foreground transition-colors hover:bg-surface-3 [&.is-selected]:bg-surface-3"
				class:is-selected={index === selectedIndex}
				onmouseenter={() => onhover(index)}
				onclick={() => onselect(item)}
			>
				<span class="flex shrink-0 items-center text-subtle-foreground"><FileText size={16} /></span
				>
				<div class="flex min-w-0 flex-col overflow-hidden">
					<span class="min-w-0 truncate">{item.title}</span>
				</div>
			</button>
		{/each}
	{/if}
</div>
