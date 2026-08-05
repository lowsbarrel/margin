<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import {
		listSnapshots,
		readSnapshot,
		deleteSnapshot,
		clearSnapshots,
		saveSnapshot,
		type Snapshot
	} from '$lib/history/bridge';
	import { writeFileBytes, readFileBytes } from '$lib/fs/bridge';
	import { editor as editorStore } from '$lib/stores/editor.svelte';
	import { IconButton } from '$lib/ui';
	import { History, Trash2, RotateCcw, X, Clock } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		filePath: string;
		onclose: () => void;
		onrestore?: (content: string) => void;
	}

	let { filePath, onclose, onrestore }: Props = $props();

	// Cap the in-DOM snapshot preview so very large notes don't lay out hundreds
	// of KB into a single text node on every preview toggle.
	const PREVIEW_MAX_CHARS = 20_000;

	let snapshots = $state<Snapshot[]>([]);
	let loading = $state(true);
	let previewContent = $state<string | null>(null);
	let previewFilename = $state<string | null>(null);

	// Generation token to discard stale async results when filePath changes
	// rapidly (e.g. fast tab switching with the history panel open).
	let loadGeneration = 0;

	$effect(() => {
		if (filePath && vault.vaultPath) {
			loadSnapshots();
		}
	});

	async function loadSnapshots() {
		if (!vault.vaultPath) return;
		const generation = ++loadGeneration;
		const requestedPath = filePath;
		loading = true;
		try {
			const result = await listSnapshots(vault.vaultPath, requestedPath);
			if (generation !== loadGeneration) return;
			snapshots = result;
		} catch (err) {
			if (generation !== loadGeneration) return;
			console.error('Failed to list snapshots:', err);
			toast.error(`${m.history_load_failed()}`);
		} finally {
			if (generation === loadGeneration) loading = false;
		}
	}

	async function handlePreview(snapshot: Snapshot) {
		if (!vault.vaultPath) return;
		if (previewFilename === snapshot.filename) {
			previewContent = null;
			previewFilename = null;
			return;
		}
		try {
			const bytes = await readSnapshot(vault.vaultPath, filePath, snapshot.filename);
			const decoded = new TextDecoder().decode(bytes);
			previewContent =
				decoded.length > PREVIEW_MAX_CHARS
					? `${decoded.slice(0, PREVIEW_MAX_CHARS)}\n\n… (preview truncated)`
					: decoded;
			previewFilename = snapshot.filename;
		} catch (err) {
			console.error('Failed to read snapshot:', err);
			toast.error(`${m.history_read_failed()}`);
		}
	}

	async function handleRestore(snapshot: Snapshot) {
		if (!vault.vaultPath) return;
		try {
			try {
				const currentBytes = await readFileBytes(filePath);
				await saveSnapshot(vault.vaultPath, filePath, currentBytes);
			} catch (err) {
				console.warn('Pre-restore snapshot failed:', err);
			}

			const bytes = await readSnapshot(vault.vaultPath, filePath, snapshot.filename);
			const content = new TextDecoder().decode(bytes);

			await writeFileBytes(filePath, new TextEncoder().encode(content));
			editorStore.setDirty(false);

			onrestore?.(content);
			await loadSnapshots(); // Refresh list to show the pre-restore snapshot
			toast.success(m.history_restored());
		} catch (err) {
			console.error('Restore failed:', err);
			toast.error(`${m.history_restore_failed()}`);
		}
	}

	async function handleDelete(snapshot: Snapshot) {
		if (!vault.vaultPath) return;
		try {
			await deleteSnapshot(vault.vaultPath, filePath, snapshot.filename);
			snapshots = snapshots.filter((s) => s.filename !== snapshot.filename);
			if (previewFilename === snapshot.filename) {
				previewContent = null;
				previewFilename = null;
			}
		} catch (err) {
			console.error('Delete failed:', err);
			toast.error(`${m.history_delete_failed()}`);
		}
	}

	async function handleClearAll() {
		if (!vault.vaultPath) return;
		try {
			const count = await clearSnapshots(vault.vaultPath, filePath);
			snapshots = [];
			previewContent = null;
			previewFilename = null;
			toast.success(`${m.history_cleared({ count: String(count) })}`);
		} catch (err) {
			console.error('Clear failed:', err);
		}
	}

	function formatDate(timestamp: number): string {
		const date = new Date(timestamp * 1000);
		const now = new Date();
		const diff = now.getTime() - date.getTime();

		if (diff < 86400000 && date.getDate() === now.getDate()) {
			return date.toLocaleTimeString(undefined, {
				hour: '2-digit',
				minute: '2-digit'
			});
		}

		// Built by construction rather than by mutating a copy of `now`: the
		// day-before-the-first is handled by the Date constructor's own rollover,
		// and an immutable Date can't drift out of step with the value it was
		// derived from.
		const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
		if (date.getDate() === yesterday.getDate() && date.getMonth() === yesterday.getMonth()) {
			return `${m.history_yesterday()} ${date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })}`;
		}

		if (date.getFullYear() === now.getFullYear()) {
			return date.toLocaleDateString(undefined, {
				month: 'short',
				day: 'numeric',
				hour: '2-digit',
				minute: '2-digit'
			});
		}

		return date.toLocaleDateString(undefined, {
			year: 'numeric',
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	function formatSize(bytes: number): string {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / 1048576).toFixed(1)} MB`;
	}

	function groupByDay(items: Snapshot[]): { label: string; snapshots: Snapshot[] }[] {
		// The result array *is* the group list, in first-seen order; the lookup
		// alongside it only points at buckets already in that array. A
		// null-prototype record keeps arbitrary date labels from colliding with
		// Object.prototype keys.
		const groups: { label: string; snapshots: Snapshot[] }[] = [];
		const byLabel: Record<string, Snapshot[]> = Object.create(null);

		for (const snap of items) {
			const date = new Date(snap.timestamp * 1000);
			const label = date.toLocaleDateString(undefined, {
				year: 'numeric',
				month: 'long',
				day: 'numeric'
			});
			let bucket = byLabel[label];
			if (!bucket) {
				bucket = [];
				byLabel[label] = bucket;
				groups.push({ label, snapshots: bucket });
			}
			bucket.push(snap);
		}

		return groups;
	}

	let grouped = $derived(groupByDay(snapshots));
</script>

<div class="flex h-full w-75 max-w-100 min-w-60 flex-col border-l border-border bg-surface-1">
	<div class="flex shrink-0 items-center justify-between border-b border-border px-3 py-2">
		<div class="flex items-center gap-2 text-muted-foreground">
			<History size={14} />
			<span class="text-sm font-semibold tracking-wide uppercase">{m.history_title()}</span>
			{#if snapshots.length > 0}
				<span
					class="rounded-full bg-surface-2 px-1.5 py-px text-xs font-medium text-subtle-foreground"
					>{snapshots.length}</span
				>
			{/if}
		</div>
		<div class="flex items-center gap-0.5">
			{#if snapshots.length > 0}
				<IconButton
					icon={Trash2}
					size="sm"
					onclick={handleClearAll}
					title={m.history_clear_all()}
				/>
			{/if}
			<IconButton icon={X} size="sm" onclick={onclose} title={m.history_close()} />
		</div>
	</div>

	<div class="flex-1 overflow-y-auto p-2">
		{#if loading}
			<div class="flex h-25 items-center justify-center text-sm text-subtle-foreground">
				{m.history_loading()}
			</div>
		{:else if snapshots.length === 0}
			<div class="flex h-25 items-center justify-center text-sm text-subtle-foreground">
				{m.history_empty()}
			</div>
		{:else}
			{#each grouped as group (group.label)}
				<div class="mb-3">
					<div
						class="mb-1 px-2 py-1 text-xs font-semibold tracking-wide text-subtle-foreground uppercase"
					>
						{group.label}
					</div>
					{#each group.snapshots as snapshot (snapshot.filename)}
						<!-- `!` counters `src/app.css`'s unlayered bare-`button` padding/radius. -->
						<button
							class="group flex w-full items-center justify-between rounded-xs p-2 text-left text-sm transition-colors hover:bg-surface-3 hover:text-foreground {previewFilename ===
							snapshot.filename
								? 'bg-surface-2 text-foreground'
								: 'text-muted-foreground'}"
							onclick={() => handlePreview(snapshot)}
						>
							<div class="flex min-w-0 flex-1 items-center gap-2">
								<Clock size={12} />
								<span class="whitespace-nowrap">{formatDate(snapshot.timestamp)}</span>
								<span class="text-xs whitespace-nowrap text-subtle-foreground"
									>{formatSize(snapshot.size)}</span
								>
							</div>
							<div
								class="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100"
								role="presentation"
								onclick={(e) => e.stopPropagation()}
								onkeydown={(e) => e.stopPropagation()}
							>
								<IconButton
									icon={RotateCcw}
									size="sm"
									onclick={() => handleRestore(snapshot)}
									title={m.history_restore()}
								/>
								<IconButton
									icon={Trash2}
									size="sm"
									onclick={() => handleDelete(snapshot)}
									title={m.history_delete()}
								/>
							</div>
						</button>

						{#if previewFilename === snapshot.filename && previewContent !== null}
							<div
								class="mx-2 my-1 max-h-75 overflow-auto rounded-sm border border-border bg-background p-2"
							>
								<pre
									class="m-0 font-mono text-xs leading-normal wrap-break-word whitespace-pre-wrap text-muted-foreground">{previewContent}</pre>
							</div>
						{/if}
					{/each}
				</div>
			{/each}
		{/if}
	</div>
</div>
