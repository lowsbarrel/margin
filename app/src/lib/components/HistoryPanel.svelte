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
	import { flushEditorWrites } from '$lib/fs/writeQueue';
	import { editor as editorStore } from '$lib/stores/editor.svelte';
	import { panes } from '$lib/stores/panes.svelte';
	import { diffLines, countChanges, type DiffLine } from '$lib/utils/line-diff';
	import { IconButton } from '$lib/ui';
	import { RotateCcwClock, Trash2, RotateCcw, X, Clock } from '@lucide/svelte';
	import { formatBytes } from '$lib/utils/bytes';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		filePath: string;
		onclose: () => void;
		onrestore?: (content: string) => void;
	}

	let { filePath, onclose, onrestore }: Props = $props();

	let snapshots = $state<Snapshot[]>([]);
	let loading = $state(true);
	/** The snapshot open below the list, and its comparison against the file. */
	let previewFilename = $state<string | null>(null);
	/** null while nothing is open or the two versions are too large to compare. */
	let diff = $state<DiffLine[] | null>(null);

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
			toast.error(m.history_load_failed());
		} finally {
			if (generation === loadGeneration) loading = false;
		}
	}

	/**
	 * Show a snapshot against what the note holds now. The whole snapshot is
	 * decoded and diffed — truncating it would hide the change the user is
	 * looking for, which is usually near the end of a long note.
	 */
	async function handlePreview(snapshot: Snapshot) {
		if (!vault.vaultPath) return;
		if (previewFilename === snapshot.filename) {
			clearPreview();
			return;
		}
		const generation = loadGeneration;
		try {
			const bytes = await readSnapshot(vault.vaultPath, filePath, snapshot.filename);
			if (generation !== loadGeneration) return;
			const decoded = new TextDecoder().decode(bytes);
			let current = '';
			try {
				current = new TextDecoder().decode(await readFileBytes(filePath));
			} catch (err) {
				console.warn('Could not read the current note for the diff:', err);
			}
			if (generation !== loadGeneration) return;
			diff = diffLines(decoded, current);
			previewFilename = snapshot.filename;
		} catch (err) {
			console.error('Failed to read snapshot:', err);
			toast.error(m.history_read_failed());
		}
	}

	function clearPreview() {
		previewFilename = null;
		diff = null;
	}

	async function handleRestore(snapshot: Snapshot) {
		if (!vault.vaultPath) return;
		try {
			// Land the debounced edit and the write queue before reading the file:
			// the pre-restore snapshot must hold the text the user actually has,
			// not a version up to one debounce behind it.
			await flushEditorWrites();
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

			// Every pane holding this note, not just the active one: the others
			// would otherwise keep the pre-restore text and overwrite the restore.
			panes.applyRestoredContent(filePath, content);
			onrestore?.(content);
			await loadSnapshots(); // Refresh list to show the pre-restore snapshot
			toast.success(m.history_restored());
		} catch (err) {
			console.error('Restore failed:', err);
			toast.error(m.history_restore_failed());
		}
	}

	async function handleDelete(snapshot: Snapshot) {
		if (!vault.vaultPath) return;
		try {
			await deleteSnapshot(vault.vaultPath, filePath, snapshot.filename);
			snapshots = snapshots.filter((s) => s.filename !== snapshot.filename);
			if (previewFilename === snapshot.filename) clearPreview();
		} catch (err) {
			console.error('Delete failed:', err);
			toast.error(m.history_delete_failed());
		}
	}

	async function handleClearAll() {
		if (!vault.vaultPath) return;
		try {
			const count = await clearSnapshots(vault.vaultPath, filePath);
			snapshots = [];
			clearPreview();
			toast.success(m.history_cleared({ count: String(count) }));
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
			<RotateCcwClock size={14} />
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
									>{formatBytes(snapshot.size)}</span
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

						{#if previewFilename === snapshot.filename}
							<div class="mx-2 my-1 overflow-hidden rounded-sm border border-border bg-background">
								<div
									class="flex items-center justify-between gap-2 border-b border-border px-2 py-1 text-xs text-subtle-foreground"
								>
									<span>{m.history_diff_current()}</span>
									{#if diff}
										{@const changes = countChanges(diff)}
										<span class="flex shrink-0 items-center gap-1.5 tabular-nums">
											<span class="text-positive">+{changes.added}</span>
											<span class="text-destructive">−{changes.removed}</span>
										</span>
									{/if}
								</div>
								{#if diff}
									<!-- The whole snapshot, line numbered: a change near the end of a
									     long note is what the user came here to find. -->
									<div class="max-h-100 overflow-auto py-1 font-mono text-xs leading-normal">
										{#each diff as line, i (i)}
											<div
												class={line.kind === 'added'
													? 'bg-surface-2 text-positive'
													: line.kind === 'removed'
														? 'bg-surface-2 text-destructive'
														: 'text-muted-foreground'}
											>
												<span
													class="inline-block w-9 shrink-0 pr-1.5 text-right text-subtle-foreground tabular-nums select-none"
													>{line.before ?? ''}</span
												>
												<span
													class="inline-block w-9 shrink-0 pr-1.5 text-right text-subtle-foreground tabular-nums select-none"
													>{line.after ?? ''}</span
												>
												<span class="whitespace-pre"
													>{line.kind === 'added'
														? '+ '
														: line.kind === 'removed'
															? '- '
															: '  '}{line.text}</span
												>
											</div>
										{/each}
									</div>
								{:else}
									<div class="px-2 py-1 text-xs text-subtle-foreground">
										{m.history_diff_too_large()}
									</div>
								{/if}
							</div>
						{/if}
					{/each}
				</div>
			{/each}
		{/if}
	</div>
</div>
