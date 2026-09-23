<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { GlassModal, Button, IconButton } from '$lib/ui';
	import {
		deleteTrash,
		emptyTrash,
		listTrash,
		restoreTrash,
		type TrashItem
	} from '$lib/history/bridge';
	import { FileText, Folder, History, RotateCcw, Trash2, X } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		onclose: () => void;
		/** Called with the vault-relative path an item was restored onto. */
		onrestored?: (path: string) => void;
	}

	let { onclose, onrestored }: Props = $props();

	let items = $state<TrashItem[]>([]);
	let loading = $state(true);
	/** The item awaiting a second click, or 'all' for the Empty trash button. */
	let confirming = $state<string | null>(null);
	let busyId = $state<string | null>(null);
	let emptying = $state(false);

	$effect(() => {
		load();
	});

	async function load() {
		if (!vault.vaultPath) return;
		loading = true;
		try {
			items = await listTrash(vault.vaultPath);
		} catch (err) {
			console.error('Failed to list trash:', err);
			toast.error(m.trash_load_failed());
		} finally {
			loading = false;
		}
	}

	async function handleRestore(item: TrashItem) {
		if (!vault.vaultPath || busyId) return;
		busyId = item.id;
		try {
			const path = await restoreTrash(vault.vaultPath, item.id);
			items = items.filter((i) => i.id !== item.id);
			toast.success(m.trash_restored({ name: item.name }));
			onrestored?.(path);
		} catch (err) {
			console.error('Restore failed:', err);
			toast.error(m.trash_restore_failed({ error: String(err) }));
		} finally {
			busyId = null;
		}
	}

	async function handleDelete(item: TrashItem) {
		if (!vault.vaultPath || busyId) return;
		if (confirming !== item.id) {
			confirming = item.id;
			return;
		}
		confirming = null;
		busyId = item.id;
		try {
			await deleteTrash(vault.vaultPath, item.id);
			items = items.filter((i) => i.id !== item.id);
		} catch (err) {
			console.error('Delete failed:', err);
			toast.error(m.trash_delete_failed({ error: String(err) }));
		} finally {
			busyId = null;
		}
	}

	async function handleEmpty() {
		if (!vault.vaultPath || emptying) return;
		if (confirming !== 'all') {
			confirming = 'all';
			return;
		}
		confirming = null;
		emptying = true;
		try {
			const count = await emptyTrash(vault.vaultPath);
			items = [];
			toast.success(m.trash_emptied({ count: String(count) }));
		} catch (err) {
			console.error('Empty trash failed:', err);
			toast.error(m.trash_empty_failed({ error: String(err) }));
		} finally {
			emptying = false;
		}
	}

	/** Vault-relative folder an item sat in, e.g. "notes/sub" ("" for the root). */
	function folderOf(path: string): string {
		const cut = path.lastIndexOf('/');
		return cut === -1 ? '' : path.slice(0, cut);
	}

	function deletedAgo(deletedAt: number | null): string {
		if (deletedAt == null) return '';
		const minutes = Math.floor((Date.now() - deletedAt) / 60_000);
		if (minutes < 1) return m.trash_deleted_just_now();
		if (minutes < 60) return m.trash_deleted_minutes({ count: String(minutes) });
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return m.trash_deleted_hours({ count: String(hours) });
		return m.trash_deleted_days({ count: String(Math.floor(hours / 24)) });
	}
</script>

<GlassModal title={m.trash_title()} {onclose} width="620px">
	{#if loading}
		<div class="flex h-24 items-center justify-center text-sm text-subtle-foreground">
			{m.trash_loading()}
		</div>
	{:else if items.length === 0}
		<div class="flex h-24 flex-col items-center justify-center gap-2 text-subtle-foreground">
			<Trash2 size={20} />
			<span class="text-sm">{m.trash_empty_state()}</span>
		</div>
	{:else}
		<div class="flex max-h-[55vh] flex-col gap-1 overflow-y-auto">
			{#each items as item (item.id)}
				<div
					class="flex items-center gap-2 rounded-sm border border-border bg-surface-1 px-2.5 py-2"
				>
					{#if item.is_dir}
						<Folder size={14} class="shrink-0 text-subtle-foreground" />
					{:else}
						<FileText size={14} class="shrink-0 text-subtle-foreground" />
					{/if}
					<div class="flex min-w-0 flex-1 flex-col">
						<span class="truncate text-sm text-foreground" title={item.path}>{item.name}</span>
						<span class="flex items-center gap-1.5 truncate text-xs text-subtle-foreground">
							<span class="truncate">{folderOf(item.path) || '/'}</span>
							{#if deletedAgo(item.deleted_at)}
								<span class="text-hairline">·</span>
								<span class="shrink-0">{deletedAgo(item.deleted_at)}</span>
							{/if}
							{#if item.has_history}
								<span class="flex shrink-0 items-center gap-0.5" title={m.trash_includes_history()}>
									<History size={11} />
								</span>
							{/if}
						</span>
					</div>

					{#if confirming === item.id}
						<span class="shrink-0 text-xs text-destructive">{m.trash_delete_confirm()}</span>
						<Button size="sm" variant="danger" onclick={() => handleDelete(item)}>
							{m.trash_confirm()}
						</Button>
						<IconButton
							icon={X}
							size="sm"
							onclick={() => (confirming = null)}
							title={m.trash_cancel()}
						/>
					{:else}
						<IconButton
							icon={RotateCcw}
							size="sm"
							disabled={busyId !== null}
							onclick={() => handleRestore(item)}
							title={m.trash_restore()}
						/>
						<IconButton
							icon={Trash2}
							size="sm"
							disabled={busyId !== null}
							onclick={() => handleDelete(item)}
							title={m.trash_delete_forever()}
						/>
					{/if}
				</div>
			{/each}
		</div>

		<div class="mt-1 flex items-center justify-between gap-2 border-t border-border pt-3">
			<span class="text-xs text-subtle-foreground">{items.length}</span>
			{#if confirming === 'all'}
				<span class="text-xs text-destructive">{m.trash_empty_confirm()}</span>
				<Button size="sm" variant="danger" loading={emptying} onclick={handleEmpty}>
					{m.trash_confirm()}
				</Button>
				<Button size="sm" variant="ghost" onclick={() => (confirming = null)}>
					{m.trash_cancel()}
				</Button>
			{:else}
				<Button size="sm" variant="danger" icon={Trash2} onclick={handleEmpty}>
					{m.trash_empty_trash()}
				</Button>
			{/if}
		</div>
	{/if}
</GlassModal>
