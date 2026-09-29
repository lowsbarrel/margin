<script lang="ts">
	import { FileText, Hash, Replace } from '@lucide/svelte';
	import { displayName } from '$lib/utils/filename';
	import { baseName } from '$lib/utils/path';
	import { displayPath, splitHighlight } from '$lib/utils/sidebar-ops';
	import * as m from '$lib/paraglide/messages.js';
	import { fileIcon, type SpotlightItem } from './spotlight-items';

	interface Props {
		item: SpotlightItem;
		index: number;
		selected: boolean;
		query: string;
		vaultPath: string | null;
		showReplace: boolean;
		onactivate: () => void;
		onhover: () => void;
		onreplace: (path: string) => void;
	}

	let {
		item,
		index,
		selected,
		query,
		vaultPath,
		showReplace,
		onactivate,
		onhover,
		onreplace
	}: Props = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	data-index={index}
	role="option"
	tabindex="-1"
	aria-selected={selected}
	onclick={onactivate}
	onmouseenter={onhover}
	class="relative flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-1.5 transition-colors {selected
		? 'bg-accent'
		: ''}"
>
	{#if selected}
		<span class="absolute top-2 bottom-2 left-0 w-0.5 rounded-full bg-brand"></span>
	{/if}

	{#if item.kind === 'tag'}
		<Hash
			size={16}
			class="shrink-0 {selected ? 'text-accent-foreground' : 'text-subtle-foreground'}"
		/>
		<span class="min-w-0 truncate font-mono text-sm text-foreground">#{item.tag}</span>
		<span class="ml-auto shrink-0 text-xs text-subtle-foreground">{item.count}</span>
	{:else}
		{@const Icon = item.kind === 'name' ? fileIcon(item.path) : FileText}
		{@const name = item.kind === 'name' ? item.entry.name : baseName(item.path)}
		{@const folder = displayPath(item.path, vaultPath)}
		<Icon
			size={16}
			class="shrink-0 self-start {selected
				? 'text-accent-foreground'
				: 'text-subtle-foreground'} mt-0.5"
		/>
		<div class="flex min-w-0 flex-1 flex-col gap-0.5">
			<div class="flex min-w-0 items-baseline gap-2">
				<span class="min-w-0 truncate text-sm font-medium text-foreground">{displayName(name)}</span
				>
				{#if folder}
					<span class="ml-auto min-w-0 shrink truncate text-xs text-subtle-foreground"
						>{folder}</span
					>
				{/if}
			</div>
			{#if item.kind === 'content' && item.hit.snippet}
				<span class="min-w-0 truncate text-xs text-muted-foreground">
					{#each splitHighlight(item.hit.snippet, query) as seg, si (si)}{#if seg.match}<mark
								class="rounded-xs bg-(--color-brand-32) px-0.5 font-medium text-foreground"
								>{seg.text}</mark
							>{:else}{seg.text}{/if}{/each}
				</span>
			{/if}
		</div>
		{#if item.kind === 'content' && showReplace}
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<span
				role="button"
				tabindex="-1"
				onclick={(e) => {
					e.stopPropagation();
					onreplace(item.path);
				}}
				title={m.spotlight_replace_all_in_file()}
				class="flex size-6 shrink-0 items-center justify-center self-center rounded-xs text-subtle-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
			>
				<Replace size={12} />
			</span>
		{/if}
	{/if}
</div>
