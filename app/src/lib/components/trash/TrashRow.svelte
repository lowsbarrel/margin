<script lang="ts">
	import { FileText, Folder, History, RotateCcw, Trash2 } from '@lucide/svelte';
	import { Button, IconButton } from '$lib/ui';
	import { splitHighlight } from '$lib/utils/sidebar-ops';
	import type { TrashItem } from '$lib/history/bridge';
	import * as m from '$lib/paraglide/messages.js';
	import { PURGE_HINT_DAYS, daysUntilPurge, folderOf, relativeTime } from './trash-model';

	interface Props {
		item: TrashItem;
		index: number;
		selected: boolean;
		query: string;
		now: number;
		busy: boolean;
		onselect: () => void;
		onrestore: (item: TrashItem) => void;
		ondelete: (item: TrashItem) => void;
	}

	let { item, index, selected, query, now, busy, onselect, onrestore, ondelete }: Props = $props();

	const folder = $derived(folderOf(item.path));
	const relative = $derived(relativeTime(item.deleted_at, now));
	const purgeDays = $derived(daysUntilPurge(item.deleted_at, now));
</script>

{#snippet marked(text: string)}
	{#each splitHighlight(text, query) as part, i (i)}
		{#if part.match}<mark
				class="rounded-xs bg-(--color-brand-32) px-0.5 font-medium text-foreground"
				>{part.text}</mark
			>{:else}{part.text}{/if}
	{/each}
{/snippet}

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	role="option"
	data-index={index}
	tabindex="-1"
	aria-selected={selected}
	class="group relative flex cursor-default items-center gap-2.5 rounded-md py-1.5 pr-2 pl-2.5 transition-colors {selected
		? 'bg-accent'
		: 'hover:bg-surface-1'}"
	onclick={onselect}
>
	{#if selected}
		<span class="absolute top-2 bottom-2 left-0 w-0.5 rounded-full bg-brand"></span>
	{/if}

	{#if item.is_dir}
		<Folder size={16} class="shrink-0 text-subtle-foreground" />
	{:else}
		<FileText size={16} class="shrink-0 text-subtle-foreground" />
	{/if}

	<div class="flex min-w-0 flex-1 flex-col gap-0.5">
		<div class="flex min-w-0 items-center gap-1.5">
			<span class="min-w-0 truncate text-sm text-foreground">{@render marked(item.name)}</span>
			{#if item.has_history}
				<span
					class="flex size-4 shrink-0 items-center justify-center rounded-xs bg-surface-2 text-subtle-foreground"
					title={m.trash_includes_history()}
				>
					<History size={10} />
				</span>
			{/if}
		</div>
		<div class="flex min-w-0 items-center gap-1.5 text-xs text-subtle-foreground">
			<span class="min-w-0 truncate"
				>{#if folder}{@render marked(folder)}{:else}{m.trash_vault_root()}{/if}</span
			>
			{#if relative}
				<span class="shrink-0">·</span>
				<span class="shrink-0 whitespace-nowrap">
					{#if relative.unit === 'now'}
						{m.trash_deleted_just_now()}
					{:else if relative.unit === 'minute'}
						{m.trash_deleted_minutes({ count: String(relative.count) })}
					{:else if relative.unit === 'hour'}
						{m.trash_deleted_hours({ count: String(relative.count) })}
					{:else}
						{m.trash_deleted_days({ count: String(relative.count) })}
					{/if}
				</span>
			{/if}
			{#if purgeDays != null && purgeDays <= PURGE_HINT_DAYS}
				<span class="shrink-0 whitespace-nowrap text-(--color-text-warning)">
					{#if purgeDays === 0}
						{m.trash_purge_today()}
					{:else if purgeDays === 1}
						{m.trash_purge_one()}
					{:else}
						{m.trash_purge_days({ days: String(purgeDays) })}
					{/if}
				</span>
			{/if}
		</div>
	</div>

	<div
		class="flex shrink-0 items-center gap-1 {selected
			? 'opacity-100'
			: 'opacity-0 group-focus-within:opacity-100 group-hover:opacity-100'}"
	>
		<Button
			size="sm"
			variant="secondary"
			icon={RotateCcw}
			disabled={busy}
			onclick={(e) => {
				e.stopPropagation();
				onrestore(item);
			}}
		>
			{m.trash_restore()}
		</Button>
		<IconButton
			icon={Trash2}
			size="sm"
			disabled={busy}
			title={m.trash_delete_forever()}
			extraClass="hover:bg-destructive/12 hover:text-(--color-text-negative)"
			onclick={(e) => {
				e.stopPropagation();
				ondelete(item);
			}}
		/>
	</div>
</div>
