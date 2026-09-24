<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { Button, GlassModal } from '$lib/ui';
	import {
		deleteTrash,
		emptyTrash,
		listTrash,
		restoreTrash,
		type TrashItem
	} from '$lib/history/bridge';
	import { Search, SearchX, Trash2, X } from '@lucide/svelte';
	import { getLocale } from '$lib/paraglide/runtime.js';
	import * as m from '$lib/paraglide/messages.js';
	import TrashConfirm from './trash/TrashConfirm.svelte';
	import TrashRow from './trash/TrashRow.svelte';
	import { filterItems, formatDay, groupItems, toRows, type GroupLabel } from './trash/trash-model';

	const OPEN_TOAST_MS = 6000;

	interface Props {
		onclose: () => void;
		onrestored?: (path: string) => void;
		onopen?: (path: string) => void;
	}

	let { onclose, onrestored, onopen }: Props = $props();

	let items = $state<TrashItem[]>([]);
	let loading = $state(true);
	let busyId = $state<string | null>(null);
	let emptying = $state(false);
	let pendingDelete = $state<TrashItem | null>(null);
	let confirmingEmpty = $state(false);
	let query = $state('');
	let selected = $state(0);
	let now = $state(Date.now());
	let searchEl = $state<HTMLInputElement | null>(null);
	let listEl = $state<HTMLDivElement | null>(null);

	const filtered = $derived(filterItems(items, query));
	const rows = $derived(toRows(groupItems(filtered, now)));
	const active = $derived(Math.min(selected, Math.max(0, filtered.length - 1)));
	const current = $derived(filtered[active] ?? null);
	const busy = $derived(busyId !== null);

	$effect(() => {
		void load();
	});

	async function load() {
		if (!vault.vaultPath) return;
		loading = true;
		try {
			items = await listTrash(vault.vaultPath);
			now = Date.now();
		} catch (err) {
			console.error('Failed to list trash:', err);
			toast.error(m.trash_load_failed());
		} finally {
			loading = false;
		}
	}

	async function restore(item: TrashItem) {
		const vaultPath = vault.vaultPath;
		if (!vaultPath || busyId) return;
		busyId = item.id;
		try {
			// trash_restore answers with a vault-relative path; the tree and the panes take absolute ones
			const restored = `${vaultPath}/${await restoreTrash(vaultPath, item.id)}`;
			items = items.filter((i) => i.id !== item.id);
			onrestored?.(restored);
			const message = m.trash_restored({ name: item.name });
			if (item.is_dir) toast.success(message);
			else
				toast.push(message, 'success', OPEN_TOAST_MS, {
					label: m.trash_open(),
					onClick: () => onopen?.(restored)
				});
		} catch (err) {
			console.error('Restore failed:', err);
			toast.error(m.trash_restore_failed({ error: String(err) }));
		} finally {
			busyId = null;
		}
	}

	async function removeForever(item: TrashItem) {
		const vaultPath = vault.vaultPath;
		if (!vaultPath || busyId) return;
		busyId = item.id;
		try {
			await deleteTrash(vaultPath, item.id);
			items = items.filter((i) => i.id !== item.id);
			toast.success(m.trash_deleted_forever({ name: item.name }));
		} catch (err) {
			console.error('Delete failed:', err);
			toast.error(m.trash_delete_failed({ error: String(err) }));
		} finally {
			busyId = null;
			pendingDelete = null;
		}
	}

	async function emptyAll() {
		const vaultPath = vault.vaultPath;
		if (!vaultPath || emptying) return;
		emptying = true;
		try {
			const count = await emptyTrash(vaultPath);
			items = [];
			toast.success(m.trash_emptied({ count: String(count) }));
		} catch (err) {
			console.error('Empty trash failed:', err);
			toast.error(m.trash_empty_failed({ error: String(err) }));
		} finally {
			emptying = false;
			confirmingEmpty = false;
		}
	}

	function confirmPending() {
		const item = pendingDelete;
		if (item) void removeForever(item);
		else void emptyAll();
	}

	function move(delta: number) {
		const last = filtered.length - 1;
		if (last < 0) return;
		selected = Math.min(Math.max(active + delta, 0), last);
		scrollToSelected();
	}

	function scrollToSelected() {
		void tick().then(() =>
			listEl?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' })
		);
	}

	function focusSearch() {
		searchEl?.focus();
		const end = searchEl?.value.length ?? 0;
		searchEl?.setSelectionRange(end, end);
	}

	function clearSearch() {
		query = '';
		selected = 0;
		focusSearch();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (pendingDelete || confirmingEmpty) return;
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			move(e.key === 'ArrowDown' ? 1 : -1);
			return;
		}
		if (e.key === 'Enter') {
			if ((e.target as HTMLElement | null)?.closest('button')) return;
			e.preventDefault();
			if (current) void restore(current);
			return;
		}
		if ((e.metaKey || e.ctrlKey) && e.key === 'Backspace') {
			e.preventDefault();
			pendingDelete = current;
			return;
		}
		if (
			e.key.length === 1 &&
			!e.metaKey &&
			!e.ctrlKey &&
			!e.altKey &&
			document.activeElement !== searchEl
		) {
			e.preventDefault();
			query += e.key;
			selected = 0;
			focusSearch();
		}
	}

	function groupLabel(label: GroupLabel, day: number | null): string {
		if (label === 'today') return m.trash_group_today();
		if (label === 'yesterday') return m.trash_group_yesterday();
		if (label === 'unknown' || day == null) return m.trash_group_unknown();
		return formatDay(day, getLocale(), now);
	}

	onMount(() => {
		// the dialog's focus trap grabs its first tabbable (the close button) one frame after mount, so search takes focus on the frame after that
		let frame = requestAnimationFrame(() => {
			frame = requestAnimationFrame(() => focusSearch());
		});
		return () => cancelAnimationFrame(frame);
	});
</script>

<svelte:document onkeydown={handleKeydown} />

<GlassModal title={m.trash_title()} {onclose} width="640px">
	{#if loading}
		<div class="flex h-40 items-center justify-center text-sm text-subtle-foreground">
			{m.trash_loading()}
		</div>
	{:else if items.length === 0}
		<div class="flex h-40 flex-col items-center justify-center gap-2 text-subtle-foreground">
			<Trash2 size={20} />
			<span class="text-sm text-foreground">{m.trash_empty_state()}</span>
			<span class="text-xs">{m.trash_empty_state_hint()}</span>
		</div>
	{:else}
		<div class="flex flex-col gap-3">
			<div class="relative flex items-center">
				<span
					class="pointer-events-none absolute left-2.5 flex items-center text-subtle-foreground"
				>
					<Search size={14} />
				</span>
				<input
					bind:this={searchEl}
					bind:value={query}
					oninput={() => (selected = 0)}
					type="text"
					spellcheck="false"
					autocomplete="off"
					placeholder={m.trash_search_placeholder()}
					aria-label={m.trash_search_placeholder()}
					class="h-8.5 w-full rounded-md border border-input bg-surface-1 pr-8 pl-8 text-sm text-foreground transition-colors outline-none placeholder:text-subtle-foreground focus:border-ring focus:bg-background focus:shadow-[0_0_0_3px_var(--color-brand-16)]"
				/>
				{#if query}
					<button
						type="button"
						onclick={clearSearch}
						title={m.trash_clear_search()}
						class="absolute right-1.5 flex size-6 items-center justify-center rounded-sm text-subtle-foreground transition-colors duration-120 hover:bg-muted hover:text-foreground"
					>
						<X size={13} />
					</button>
				{/if}
			</div>

			{#if filtered.length === 0}
				<div class="flex h-40 flex-col items-center justify-center gap-2 text-subtle-foreground">
					<SearchX size={20} />
					<span class="text-sm text-foreground">{m.trash_no_matches()}</span>
					<span class="max-w-80 truncate text-xs">
						{m.trash_no_matches_hint({ query: query.trim() })}
					</span>
				</div>
			{:else}
				<div
					bind:this={listEl}
					role="listbox"
					aria-label={m.trash_title()}
					class="flex max-h-[52vh] min-h-0 flex-col overflow-y-auto p-1.5"
				>
					{#each rows as row (row.key)}
						{#if row.kind === 'group'}
							<div
								role="presentation"
								class="px-2.5 pt-4 pb-1 text-xs font-semibold tracking-wide text-subtle-foreground uppercase first:pt-0"
							>
								{groupLabel(row.label, row.day)}
							</div>
						{:else}
							<TrashRow
								item={row.item}
								index={row.index}
								selected={row.index === active}
								{query}
								{now}
								busy={busy || emptying}
								onselect={() => (selected = row.index)}
								onrestore={restore}
								ondelete={(item) => (pendingDelete = item)}
							/>
						{/if}
					{/each}
				</div>
			{/if}

			<div class="flex items-center justify-between gap-3 border-t border-border pt-3">
				<span class="text-xs text-subtle-foreground">
					{#if query.trim()}
						{m.trash_items_filtered({
							shown: String(filtered.length),
							total: String(items.length)
						})}
					{:else if items.length === 1}
						{m.trash_items_one()}
					{:else}
						{m.trash_items_many({ count: String(items.length) })}
					{/if}
				</span>
				<Button
					size="sm"
					variant="danger"
					icon={Trash2}
					disabled={busy || emptying}
					onclick={() => (confirmingEmpty = true)}
				>
					{m.trash_empty_trash()}
				</Button>
			</div>
		</div>
	{/if}

	{#if pendingDelete}
		<TrashConfirm
			title={m.trash_delete_confirm()}
			body={m.trash_delete_confirm_body({ name: pendingDelete.name })}
			confirmLabel={m.trash_delete_forever()}
			busy={busyId === pendingDelete.id}
			onconfirm={confirmPending}
			oncancel={() => (pendingDelete = null)}
		/>
	{/if}

	{#if confirmingEmpty}
		<TrashConfirm
			title={m.trash_empty_confirm()}
			body={m.trash_empty_confirm_body()}
			confirmLabel={m.trash_empty_trash()}
			busy={emptying}
			onconfirm={confirmPending}
			oncancel={() => (confirmingEmpty = false)}
		/>
	{/if}
</GlassModal>
