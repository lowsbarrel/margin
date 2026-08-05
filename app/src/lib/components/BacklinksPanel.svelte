<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { listBacklinks, type Backlink } from '$lib/fs/bridge';
	import { IconButton } from '$lib/ui';
	import { Link2, X } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		filePath: string;
		onclose: () => void;
		onfileselect?: (path: string) => void;
	}

	let { filePath, onclose, onfileselect }: Props = $props();

	let backlinks = $state<Backlink[]>([]);
	let loading = $state(true);

	// Generation token to discard stale async results when filePath changes
	// rapidly (e.g. fast tab switching with the panel open).
	let loadGeneration = 0;

	$effect(() => {
		if (filePath && vault.vaultPath) {
			load();
		}
	});

	async function load() {
		if (!vault.vaultPath) return;
		const generation = ++loadGeneration;
		loading = true;
		try {
			const result = await listBacklinks(vault.vaultPath, filePath);
			if (generation !== loadGeneration) return;
			backlinks = result;
		} catch (err) {
			if (generation !== loadGeneration) return;
			console.error('Failed to list backlinks:', err);
			toast.error(m.backlinks_load_failed());
		} finally {
			if (generation === loadGeneration) loading = false;
		}
	}

	function noteTitle(name: string): string {
		return name.endsWith('.md') ? name.slice(0, -3) : name;
	}

	/** The folder a note sits in, so same-named notes stay distinguishable. */
	function folderOf(path: string): string {
		const root = vault.vaultPath;
		const rel = root && path.startsWith(root) ? path.slice(root.length + 1) : path;
		const cut = rel.lastIndexOf('/');
		return cut > 0 ? rel.slice(0, cut) : '';
	}
</script>

<div class="flex h-full w-75 max-w-100 min-w-60 flex-col border-l border-border bg-surface-1">
	<div class="flex shrink-0 items-center justify-between border-b border-border px-3 py-2">
		<div class="flex items-center gap-2 text-muted-foreground">
			<Link2 size={14} />
			<span class="text-sm font-semibold tracking-wide uppercase">{m.backlinks_title()}</span>
			{#if backlinks.length > 0}
				<span
					class="rounded-full bg-surface-2 px-1.5 py-px text-xs font-medium text-subtle-foreground"
					>{backlinks.length}</span
				>
			{/if}
		</div>
		<IconButton icon={X} size="sm" onclick={onclose} title={m.backlinks_close()} />
	</div>

	<div class="flex-1 overflow-y-auto p-2">
		{#if loading}
			<div class="flex h-25 items-center justify-center text-sm text-subtle-foreground">
				{m.backlinks_loading()}
			</div>
		{:else if backlinks.length === 0}
			<div
				class="flex h-25 items-center justify-center px-3 text-center text-sm text-subtle-foreground"
			>
				{m.backlinks_empty()}
			</div>
		{:else}
			{#each backlinks as link (link.path)}
				<button
					class="flex w-full flex-col items-start gap-0.5 rounded-xs p-2 text-left text-sm text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
					onclick={() => onfileselect?.(link.path)}
				>
					<span class="w-full truncate">{noteTitle(link.name)}</span>
					{#if folderOf(link.path)}
						<span class="w-full truncate text-xs text-subtle-foreground">{folderOf(link.path)}</span
						>
					{/if}
				</button>
			{/each}
		{/if}
	</div>
</div>
