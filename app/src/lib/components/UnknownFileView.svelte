<script lang="ts">
	import { FileQuestion, ExternalLink, FolderOpen } from '@lucide/svelte';
	import { openPath } from '@tauri-apps/plugin-opener';
	import { revealInFileManager } from '$lib/fs/bridge';
	import { formatBytes } from '$lib/utils/bytes';
	import { mimeForPath } from '$lib/utils/mime';
	import { toOsPath } from '$lib/editor/image-url';
	import { toast } from '$lib/stores/toast.svelte';
	import { Button } from '$lib/ui';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		path: string;
		name: string;
		size?: number;
		modified?: number;
	}

	let { path, name, size, modified }: Props = $props();

	let extension = $derived(
		name.includes('.') ? name.slice(name.lastIndexOf('.') + 1).toUpperCase() : ''
	);
	let mimeType = $derived(mimeForPath(path));
	let typeLabel = $derived(
		extension && mimeType !== 'application/octet-stream'
			? `${extension} · ${mimeType}`
			: extension || mimeType
	);
	let modifiedLabel = $derived(
		modified
			? new Date(modified * 1000).toLocaleString(undefined, {
					year: 'numeric',
					month: 'long',
					day: 'numeric',
					hour: '2-digit',
					minute: '2-digit'
				})
			: null
	);

	async function openExternally() {
		try {
			await openPath(toOsPath(path));
		} catch (err) {
			toast.error(m.toast_cannot_open_file({ error: String(err) }));
		}
	}

	async function reveal() {
		try {
			await revealInFileManager(path);
		} catch (err) {
			toast.error(m.toast_open_finder_failed({ error: String(err) }));
		}
	}
</script>

<div class="flex h-full w-full items-center justify-center overflow-auto bg-surface-2 p-6">
	<div
		class="flex w-full max-w-md flex-col items-center rounded-md border border-border bg-background p-6 text-center shadow-(--shadow-md)"
	>
		<FileQuestion size={30} class="mb-3 text-subtle-foreground" />
		<p
			class="mb-5 max-w-full truncate font-sans text-base font-semibold text-foreground"
			title={name}
		>
			{name}
		</p>
		<dl class="mb-6 grid w-full grid-cols-[auto_minmax(0,1fr)] gap-x-5 gap-y-1.5 text-left text-xs">
			<dt class="text-subtle-foreground">{m.file_info_type()}</dt>
			<dd class="truncate text-foreground">{typeLabel}</dd>
			{#if size !== undefined}
				<dt class="text-subtle-foreground">{m.file_info_size()}</dt>
				<dd class="text-foreground tabular-nums">{formatBytes(size)}</dd>
			{/if}
			{#if modifiedLabel}
				<dt class="text-subtle-foreground">{m.file_info_modified()}</dt>
				<dd class="text-foreground">{modifiedLabel}</dd>
			{/if}
		</dl>
		<div class="flex flex-wrap items-center justify-center gap-2">
			<Button variant="primary" size="sm" icon={ExternalLink} onclick={openExternally}>
				{m.viewer_open_external()}
			</Button>
			<Button variant="secondary" size="sm" icon={FolderOpen} onclick={reveal}>
				{m.viewer_reveal_in_finder()}
			</Button>
		</div>
	</div>
</div>
