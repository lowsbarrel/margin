<script lang="ts">
	import { ChevronDown, ChevronUp, Search, X } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { VIEWER_CONTROL } from './control-classes';
	import type { PdfSearch } from './pdf-search.svelte';

	interface Props {
		search: PdfSearch;
		onclose: () => void;
	}

	let { search, onclose }: Props = $props();

	let inputEl = $state<HTMLInputElement>(undefined!);

	$effect(() => {
		inputEl?.focus();
		inputEl?.select();
	});

	let countLabel = $derived(
		search.matches.length > 0
			? `${search.matchIndex + 1} / ${search.matches.length}`
			: search.query.trim()
				? search.searching
					? '…'
					: m.pdf_find_none()
				: ''
	);
</script>

<div
	class="absolute top-2 right-3 z-10 flex items-center gap-1 rounded-sm border border-border bg-background p-1 shadow-(--shadow-md)"
>
	<Search size={13} class="ml-1 shrink-0 text-subtle-foreground" />
	<input
		bind:this={inputEl}
		bind:value={search.query}
		oninput={() => void search.run()}
		placeholder={m.pdf_find_placeholder()}
		aria-label={m.pdf_find_placeholder()}
		class="w-40 bg-transparent text-xs text-foreground outline-none placeholder:text-subtle-foreground"
	/>
	<span class="min-w-16 text-right text-xs text-subtle-foreground tabular-nums">{countLabel}</span>
	<button
		class={VIEWER_CONTROL}
		onclick={() => void search.goTo(search.matchIndex - 1)}
		disabled={search.matches.length === 0}
		aria-label={m.pdf_find_previous()}
	>
		<ChevronUp size={14} />
	</button>
	<button
		class={VIEWER_CONTROL}
		onclick={() => void search.goTo(search.matchIndex + 1)}
		disabled={search.matches.length === 0}
		aria-label={m.pdf_find_next()}
	>
		<ChevronDown size={14} />
	</button>
	<button class={VIEWER_CONTROL} onclick={onclose} aria-label={m.tab_close()}>
		<X size={14} />
	</button>
</div>
