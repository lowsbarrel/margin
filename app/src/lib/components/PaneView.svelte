<script lang="ts">
	import type { Pane } from '$lib/stores/panes.svelte';
	import { panes, fileTitle, toBreadcrumbs } from '$lib/stores/panes.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { drag } from '$lib/stores/drag.svelte';
	import Editor from '$lib/components/Editor.svelte';
	import ImageViewer from '$lib/components/ImageViewer.svelte';
	import CanvasEditor from '$lib/components/CanvasEditor.svelte';
	import GraphView from '$lib/components/GraphView.svelte';
	import { X, ChevronRight, Pin } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { handleTabMouseDown } from '$lib/utils/tab-drag';

	let {
		pane,
		paneIndex,
		onfileselect,
		onrename,
		onwikilink,
		ontabcontextmenu,
		attachmentFolder,
		dropTarget,
		ondropenter,
		ondropleave
	}: {
		pane: Pane;
		paneIndex: number;
		onfileselect: (path: string, searchText?: string) => void;
		onrename: (from: string, to: string, isDir?: boolean) => void;
		onwikilink: (title: string) => void;
		ontabcontextmenu: (e: MouseEvent, paneIndex: number, tabIndex: number) => void;
		attachmentFolder: string | null;
		dropTarget: { paneIndex: number; zone: 'left' | 'center' | 'right' } | null;
		ondropenter: (paneIndex: number, zone: 'left' | 'center' | 'right') => void;
		ondropleave: (paneIndex: number, zone: 'left' | 'center' | 'right') => void;
	} = $props();

	let paneActiveTab = $derived(
		pane.activeTabIndex >= 0 && pane.activeTabIndex < pane.tabs.length
			? pane.tabs[pane.activeTabIndex]
			: null
	);

	let paneCrumbs = $derived(
		paneActiveTab ? toBreadcrumbs(paneActiveTab.path, vault.vaultPath) : []
	);

	// The active tab lifts onto the canvas colour and carries a 2px orange
	// underline. The underline is an ::after pseudo-element so it doesn't shift
	// the tab's box the way a real border would.
	const ACTIVE_TAB =
		"bg-background text-foreground after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:bg-brand after:content-['']";
	const INACTIVE_TAB = 'text-subtle-foreground hover:bg-surface-3 hover:text-muted-foreground';

	const DROP_ZONE_BASE = 'pointer-events-auto flex items-center justify-center transition-colors';

	/**
	 * Drop-zone chrome. Class names are spelled out in full rather than
	 * assembled from fragments — Tailwind scans this file as plain text, so an
	 * interpolated `border-${edge}` would never be generated.
	 */
	function dropZoneClass(zone: 'left' | 'center' | 'right', fileDrag: boolean, active: boolean) {
		if (zone === 'center') {
			return `${DROP_ZONE_BASE} flex-[40] ${active ? 'bg-brand/22' : ''}`;
		}
		if (fileDrag) {
			const edge = zone === 'left' ? 'border-r-2' : 'border-l-2';
			return `${DROP_ZONE_BASE} flex-[0_0_56px] border-solid ${edge} ${
				active ? 'border-brand bg-brand/18' : 'border-brand/30'
			}`;
		}
		const edge = zone === 'left' ? 'border-r' : 'border-l';
		return `${DROP_ZONE_BASE} flex-[30] border-dashed border-brand/40 ${edge} ${
			active ? 'bg-brand/22' : ''
		}`;
	}
</script>

<!-- `!` utilities: app.css's base `button` rule is unlayered, so it outranks
     everything Tailwind emits into `@layer utilities`. Padding and radius on a
     <button> only stick when marked important. -->

<!-- Tab Bar -->
<div class="flex h-10 min-h-10 items-center overflow-hidden border-b border-border bg-surface-1">
	<div
		class="flex flex-1 [scrollbar-width:none] overflow-x-auto overflow-y-hidden [&::-webkit-scrollbar]:hidden"
	>
		{#each pane.tabs as tab, i (tab.id)}
			<div
				class="relative flex h-10 min-w-9 shrink cursor-pointer items-center gap-1.5 border-r border-border px-2.5 text-xs font-medium tracking-normal whitespace-nowrap transition-colors select-none {i ===
				pane.activeTabIndex
					? ACTIVE_TAB
					: INACTIVE_TAB}"
				role="tab"
				tabindex={0}
				aria-selected={i === pane.activeTabIndex}
				onmousedown={(e) => {
					e.stopPropagation();
					handleTabMouseDown(e, paneIndex, i);
				}}
				oncontextmenu={(e) => ontabcontextmenu(e, paneIndex, i)}
				onkeydown={(e) => {
					if (e.key === 'Enter' || e.key === ' ') panes.switchTab(paneIndex, i);
				}}
			>
				{#if tab.pinned}
					<Pin size={11} class="shrink-0 text-accent-foreground" />
				{/if}
				<span class="max-w-40 min-w-0 shrink truncate">{fileTitle(tab.path)}</span>
				<button
					class="flex size-[18px] shrink-0 cursor-pointer items-center justify-center rounded-xs p-0 text-subtle-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
					onclick={(e) => {
						e.stopPropagation();
						panes.closeTab(paneIndex, i);
					}}
					tabindex={-1}
					aria-label={m.tab_close_label()}
				>
					<X size={12} />
				</button>
			</div>
		{/each}
	</div>
	{#if panes.list.length > 1}
		<div class="flex h-full shrink-0 items-center gap-0.5 border-l border-border px-1.5">
			<button
				class="flex size-[26px] cursor-pointer items-center justify-center rounded-sm p-0 text-subtle-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
				onclick={(e) => {
					e.stopPropagation();
					panes.closePane(paneIndex);
				}}
				title={m.pane_close()}
			>
				<X size={14} />
			</button>
		</div>
	{/if}
</div>

<!-- Breadcrumbs -->
{#if paneActiveTab && paneCrumbs.length > 0}
	<div
		class="flex min-h-[30px] items-center gap-1 overflow-x-auto border-b border-border bg-background px-4 py-1.5 text-xs whitespace-nowrap text-subtle-foreground"
	>
		<!-- Keyed on the path prefix each crumb stands for: crumb labels alone can
		     repeat within one path (`notes/ideas/notes`), but prefixes cannot. -->
		{#each paneCrumbs as crumb, i (paneCrumbs.slice(0, i + 1).join('/'))}
			{#if i > 0}
				<ChevronRight size={12} />
			{/if}
			<span
				class={i === paneCrumbs.length - 1
					? 'font-medium text-muted-foreground'
					: 'text-subtle-foreground'}>{crumb}</span
			>
		{/each}
	</div>
{/if}

<!-- Editor / Viewer -->
<main class="relative flex flex-1 flex-col overflow-hidden bg-background">
	{#if paneActiveTab}
		{#each pane.tabs.filter((t) => t.type === 'markdown') as tab (tab.id)}
			{@const isActive = tab.id === paneActiveTab?.id}
			<div class={isActive ? 'contents' : 'hidden'}>
				<Editor
					filePath={tab.path}
					initialContent={tab.content}
					externalContentVersion={pane.externalContentVersion}
					title={fileTitle(tab.path)}
					active={isActive && paneIndex === panes.activePaneIndex}
					initialCursorPos={tab.cursorPos}
					{onrename}
					{onwikilink}
					onsave={(content) => {
						tab.content = content;
						panes.broadcastContent(paneIndex, tab.path, content);
					}}
					onsnapshotcursor={(pos) => (tab.cursorPos = pos)}
					{attachmentFolder}
				/>
			</div>
		{/each}
		{#if paneActiveTab.type === 'image' && paneActiveTab.blobUrl}
			{#key paneActiveTab.id}
				<ImageViewer src={paneActiveTab.blobUrl} alt={fileTitle(paneActiveTab.path)} />
			{/key}
		{:else if paneActiveTab.type === 'pdf' && paneActiveTab.pdfData}
			{#key paneActiveTab.id}
				<!-- pdfjs-dist is the heaviest viewer dependency and most sessions never
				     open a PDF, so its chunk is fetched on demand. -->
				{#await import('$lib/components/PdfViewer.svelte') then { default: PdfViewer }}
					<PdfViewer data={paneActiveTab.pdfData} />
				{/await}
			{/key}
		{:else if paneActiveTab.type === 'canvas'}
			{#key paneActiveTab.id}
				<CanvasEditor
					filePath={paneActiveTab.path}
					initialData={paneActiveTab.content}
					onsave={(content) => {
						paneActiveTab.content = content;
						panes.broadcastContent(paneIndex, paneActiveTab.path, content);
					}}
				/>
			{/key}
		{:else if paneActiveTab.type === 'graph'}
			{#key paneActiveTab.id}
				<GraphView {onfileselect} />
			{/key}
		{/if}
	{:else}
		<div class="flex flex-1 items-center justify-center text-sm text-subtle-foreground">
			<p>{m.editor_empty_state()}</p>
		</div>
	{/if}
</main>

<!-- Drop overlay -->
{#if drag.active && !(drag.item?.kind === 'tab' && drag.item.paneIndex === paneIndex)}
	{@const fileDrag = drag.item?.kind === 'file'}
	{@const leftActive = dropTarget?.paneIndex === paneIndex && dropTarget.zone === 'left'}
	{@const centerActive = dropTarget?.paneIndex === paneIndex && dropTarget.zone === 'center'}
	{@const rightActive = dropTarget?.paneIndex === paneIndex && dropTarget.zone === 'right'}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="pointer-events-none absolute inset-0 z-[100] flex {fileDrag
			? 'justify-between'
			: 'bg-brand/6'}"
	>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class={dropZoneClass('left', fileDrag, leftActive)}
			onmouseenter={() => ondropenter(paneIndex, 'left')}
			onmouseleave={() => ondropleave(paneIndex, 'left')}
		>
			<span
				class="pointer-events-none rounded-xs border border-brand/50 bg-surface-1 px-2 py-[3px] text-xs font-medium text-accent-foreground transition-opacity {leftActive
					? 'opacity-100'
					: 'opacity-0'}">Split Left</span
			>
		</div>
		{#if drag.item?.kind === 'tab'}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class={dropZoneClass('center', fileDrag, centerActive)}
				onmouseenter={() => ondropenter(paneIndex, 'center')}
				onmouseleave={() => ondropleave(paneIndex, 'center')}
			>
				<span
					class="pointer-events-none rounded-xs border border-brand/50 bg-surface-1 px-2 py-[3px] text-xs font-medium text-accent-foreground transition-opacity {centerActive
						? 'opacity-100'
						: 'opacity-0'}">Move Here</span
				>
			</div>
		{/if}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class={dropZoneClass('right', fileDrag, rightActive)}
			onmouseenter={() => ondropenter(paneIndex, 'right')}
			onmouseleave={() => ondropleave(paneIndex, 'right')}
		>
			<span
				class="pointer-events-none rounded-xs border border-brand/50 bg-surface-1 px-2 py-[3px] text-xs font-medium text-accent-foreground transition-opacity {rightActive
					? 'opacity-100'
					: 'opacity-0'}">Split Right</span
			>
		</div>
	</div>
{/if}
