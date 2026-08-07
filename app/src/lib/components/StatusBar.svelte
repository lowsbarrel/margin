<script lang="ts">
	import { editor } from '$lib/stores/editor.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	// `pdf-export` is NOT imported statically. It pulls in mermaid, lowlight,
	// jspdf and html2canvas-pro at module scope, so a static import here put
	// several megabytes of PDF machinery into the boot chunk purely because the
	// status bar renders an export button. It is loaded on first click instead —
	// the module is cached afterwards, so only the first export pays.
	import { IconButton } from '$lib/ui';
	// The sync-status glyphs are indicators, not controls — they stay static, so
	// brushing past them with the pointer doesn't set something wiggling.
	// `Moon` has no animated counterpart in the registry.
	import { CloudOff, Loader, Check, CircleAlert, Moon, FileDown, Link2 } from '@lucide/svelte';
	import {
		Sun,
		LogOut,
		Settings,
		RefreshCw,
		ArrowLeftRight,
		History
	} from '$lib/components/movingicons';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		onlogout?: () => void;
		onsettings?: () => void;
		onsync?: () => void;
		onswitchvault?: () => void;
		onhistory?: () => void;
		historyActive?: boolean;
		onbacklinks?: () => void;
		backlinksActive?: boolean;
	}

	let {
		onlogout,
		onsettings,
		onsync,
		onswitchvault,
		onhistory,
		historyActive = false,
		onbacklinks,
		backlinksActive = false
	}: Props = $props();
	let exporting = $state(false);

	// One class string per sync state rather than a base + overrides, so two
	// colour utilities never race for the same property inside the same layer.
	let syncChipClass = $derived(
		editor.syncStatus === 'synced'
			? 'bg-surface-2 text-positive'
			: editor.syncStatus === 'syncing'
				? 'bg-accent text-accent-foreground'
				: editor.syncStatus === 'error'
					? 'bg-surface-2 text-destructive'
					: 'bg-surface-2 text-muted-foreground'
	);

	async function handleExportPdf() {
		const tiptap = editor.tiptap;
		if (!tiptap || exporting) return;

		exporting = true;
		try {
			const { exportPdf } = await import('$lib/utils/pdf-export');
			await exportPdf(tiptap, m.statusbar_export_pdf_success());
		} catch (err) {
			console.error('PDF export failed:', err);
			toast.error(m.toast_pdf_export_failed({ error: String(err) }));
		} finally {
			exporting = false;
		}
	}
</script>

<footer
	class="flex h-9 shrink-0 items-center justify-between gap-3 border-t border-border bg-surface-1 pr-2 pl-4 text-xs/5 tracking-normal text-subtle-foreground select-none"
>
	<div class="flex items-center gap-1.5">
		<span class="tabular-nums">Ln {editor.cursorLine}, Col {editor.cursorCol}</span>
		<span class="text-hairline">·</span>
		<span class="tabular-nums">{m.statusbar_markdown()}</span>
		<span class="text-hairline">·</span>
		<span class="tabular-nums">UTF-8</span>
	</div>

	<div class="flex items-center gap-1.5">
		<!-- `title` carries the failure reason. Without it the chip could only say
		     *that* sync broke, which made a genuine failure and an unconfigured
		     vault look identical. `cursor-help` advertises that hovering explains. -->
		<span
			class="flex items-center gap-[5px] rounded-full px-2 py-[3px] {syncChipClass}"
			class:cursor-help={!!editor.syncError}
			title={editor.syncError ?? undefined}
		>
			{#if editor.syncStatus === 'synced'}
				<Check size={12} />
				<span>{m.statusbar_synced()}</span>
			{:else if editor.syncStatus === 'syncing'}
				<Loader size={12} class="animate-spin" />
				<span
					>{m.statusbar_syncing()}{#if editor.syncProgress && editor.syncProgress.total > 0}&nbsp;({editor
							.syncProgress.done}/{editor.syncProgress.total}){/if}</span
				>
			{:else if editor.syncStatus === 'error'}
				<CircleAlert size={12} />
				<span>{m.statusbar_sync_error()}</span>
			{:else}
				<CloudOff size={12} />
				<span>{m.statusbar_local()}</span>
			{/if}
		</span>

		{#if onsync}
			<IconButton
				icon={RefreshCw}
				size="sm"
				onclick={onsync}
				title={m.statusbar_sync_now()}
				extraClass={editor.syncStatus === 'syncing' ? 'spin' : ''}
				disabled={editor.syncStatus === 'syncing'}
			/>
		{/if}

		{#if editor.tiptap}
			<IconButton
				icon={exporting ? Loader : FileDown}
				size="sm"
				onclick={handleExportPdf}
				title={m.statusbar_export_pdf()}
			/>
		{/if}

		{#if onbacklinks && editor.tiptap}
			<IconButton
				icon={Link2}
				size="sm"
				onclick={onbacklinks}
				title={m.statusbar_backlinks()}
				active={backlinksActive}
			/>
		{/if}

		{#if onhistory && editor.tiptap}
			<IconButton
				icon={History}
				size="sm"
				onclick={onhistory}
				title={m.statusbar_history()}
				active={historyActive}
			/>
		{/if}

		{#if onsettings}
			<IconButton icon={Settings} size="sm" onclick={onsettings} title={m.statusbar_settings()} />
		{/if}

		<IconButton
			icon={theme.current === 'dark' ? Sun : Moon}
			size="sm"
			onclick={() => theme.toggle()}
			title={m.statusbar_toggle_theme()}
		/>

		{#if onswitchvault}
			<button
				class="flex h-6 max-w-40 items-center gap-[5px] rounded-full border border-border bg-transparent px-2 py-0 text-muted-foreground hover:border-hairline hover:bg-surface-2 hover:text-foreground"
				onclick={onswitchvault}
				title={m.statusbar_switch_vault()}
			>
				<ArrowLeftRight size={12} />
				<span class="min-w-0 truncate">{vault.profileName || 'Vault'}</span>
			</button>
		{/if}

		{#if onlogout}
			<IconButton icon={LogOut} size="sm" onclick={onlogout} title={m.statusbar_lock_vault()} />
		{/if}
	</div>
</footer>
