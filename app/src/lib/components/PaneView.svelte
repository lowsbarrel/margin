<script lang="ts">
	import type { Pane } from '$lib/stores/panes.svelte';
	import { panes, fileTitle } from '$lib/stores/panes.svelte';
	import { drag } from '$lib/stores/drag.svelte';
	import Editor from '$lib/components/Editor.svelte';
	import ImageViewer from '$lib/components/ImageViewer.svelte';
	import UnknownFileView from '$lib/components/UnknownFileView.svelte';
	import CanvasEditor from '$lib/components/CanvasEditor.svelte';
	import PaneBreadcrumbs from '$lib/components/pane/PaneBreadcrumbs.svelte';
	import { X, Pin } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import {
		handleTabMouseDown,
		paneZoneMode,
		stripDropGap,
		stripGapAt,
		tabStripAccepts
	} from '$lib/utils/tab-drag';

	let {
		pane,
		paneIndex,
		focused,
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
		focused: boolean;
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

	const TAB_BASE =
		'relative flex h-10 min-w-30 shrink cursor-pointer items-center gap-1.5 border-r border-border px-2.5 text-xs font-medium tracking-normal whitespace-nowrap transition-colors select-none';
	const ACTIVE_TAB =
		"bg-background text-foreground after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:bg-brand after:content-['']";
	const ACTIVE_TAB_UNFOCUSED =
		"bg-background text-foreground after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:bg-muted-foreground after:content-['']";
	const INACTIVE_TAB = 'text-subtle-foreground hover:bg-surface-3 hover:text-muted-foreground';

	const DROP_ZONE_BASE = 'pointer-events-auto flex items-center justify-center transition-colors';

	// Tailwind scans plain text: the border-edge classes stay literal, only which one is dynamic.
	function dropZoneClass(zone: 'left' | 'center' | 'right', fileDrag: boolean, active: boolean) {
		if (zone === 'center') {
			return `${DROP_ZONE_BASE} flex-40 ${active ? 'bg-brand/22' : ''}`;
		}
		if (fileDrag) {
			const edge = zone === 'left' ? 'border-r-2' : 'border-l-2';
			return `${DROP_ZONE_BASE} flex-[0_0_56px] border-solid ${edge} ${
				active ? 'border-brand bg-brand/18' : 'border-brand/30'
			}`;
		}
		const edge = zone === 'left' ? 'border-r' : 'border-l';
		return `${DROP_ZONE_BASE} flex-30 border-dashed border-brand/40 ${edge} ${
			active ? 'bg-brand/22' : ''
		}`;
	}

	let stripEl: HTMLDivElement | undefined;
	let tabsEl: HTMLDivElement | undefined;

	$effect(() => {
		const index = pane.activeTabIndex;
		if (index < 0 || index >= pane.tabs.length) return;
		(tabsEl?.children[index] as HTMLElement | undefined)?.scrollIntoView({
			block: 'nearest',
			inline: 'nearest'
		});
	});

	function handleStripWheel(e: WheelEvent) {
		if (!tabsEl || Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
		e.preventDefault();
		tabsEl.scrollLeft += e.deltaY;
	}

	let zoneMode = $derived(paneZoneMode(drag.item, paneIndex, pane.tabs.length));
	let stripRawGap = $derived(
		drag.stripTarget?.paneIndex === paneIndex ? drag.stripTarget.index : null
	);
	let stripGap = $derived(
		stripRawGap === null ? null : stripDropGap(drag.item, paneIndex, pane.tabs, stripRawGap)
	);

	function handleStripMove(e: MouseEvent) {
		if (!stripEl || !tabStripAccepts(drag.item, paneIndex, pane.tabs.length)) return;
		if (dropTarget?.paneIndex === paneIndex) ondropleave(paneIndex, dropTarget.zone);
		drag.setStripTarget(paneIndex, stripGapAt(stripEl, e.clientX));
	}

	function handleStripLeave() {
		if (drag.stripTarget?.paneIndex === paneIndex) drag.clearStripTarget();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={stripEl}
	class="relative z-110 flex h-10 min-h-10 items-center overflow-hidden border-b border-border {stripGap ===
	null
		? 'bg-surface-1'
		: 'bg-brand/8'}"
	onmouseenter={handleStripMove}
	onmousemove={handleStripMove}
	onmouseleave={handleStripLeave}
>
	<div
		bind:this={tabsEl}
		class="flex flex-1 scrollbar-none overflow-x-auto overflow-y-hidden [&::-webkit-scrollbar]:hidden"
		role="tablist"
		onwheel={handleStripWheel}
	>
		{#each pane.tabs as tab, i (tab.id)}
			<div
				class="{TAB_BASE} {i === pane.activeTabIndex
					? focused
						? ACTIVE_TAB
						: ACTIVE_TAB_UNFOCUSED
					: INACTIVE_TAB}"
				data-tab=""
				role="tab"
				tabindex={0}
				aria-selected={i === pane.activeTabIndex}
				aria-label={fileTitle(tab.path)}
				title={fileTitle(tab.path)}
				onmousedown={(e) => {
					e.stopPropagation();
					handleTabMouseDown(e, paneIndex, i);
				}}
				onauxclick={(e) => {
					if (e.button !== 1) return;
					e.preventDefault();
					panes.closeTab(paneIndex, i);
				}}
				oncontextmenu={(e) => ontabcontextmenu(e, paneIndex, i)}
				onkeydown={(e) => {
					if (e.key === 'Enter' || e.key === ' ') panes.switchTab(paneIndex, i);
				}}
			>
				{#if stripGap === i}
					<span class="pointer-events-none absolute inset-y-1 -left-px w-0.5 rounded-full bg-brand"
					></span>
				{/if}
				{#if i === pane.tabs.length - 1 && stripGap === pane.tabs.length}
					<span class="pointer-events-none absolute inset-y-1 -right-px w-0.5 rounded-full bg-brand"
					></span>
				{/if}
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

{#if paneActiveTab}
	<PaneBreadcrumbs {paneIndex} path={paneActiveTab.path} />
{/if}

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
					viewMode={tab.viewMode}
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
				<ImageViewer
					src={paneActiveTab.blobUrl}
					name={fileTitle(paneActiveTab.path)}
					size={paneActiveTab.size}
				/>
			{/key}
		{:else if paneActiveTab.type === 'pdf' && paneActiveTab.pdfData}
			{#key paneActiveTab.id}
				{#await import('$lib/components/PdfViewer.svelte') then { default: PdfViewer }}
					<PdfViewer
						data={paneActiveTab.pdfData}
						path={paneActiveTab.path}
						name={fileTitle(paneActiveTab.path)}
					/>
				{/await}
			{/key}
		{:else if paneActiveTab.type === 'unknown'}
			{#key paneActiveTab.id}
				<UnknownFileView
					path={paneActiveTab.path}
					name={fileTitle(paneActiveTab.path)}
					size={paneActiveTab.size}
					modified={paneActiveTab.modified}
				/>
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
		{/if}
	{:else}
		<div class="flex flex-1 items-center justify-center text-sm text-subtle-foreground">
			<p>{m.editor_empty_state()}</p>
		</div>
	{/if}
</main>

{#if zoneMode !== 'none'}
	{@const fileDrag = drag.item?.kind === 'file'}
	{@const leftActive = dropTarget?.paneIndex === paneIndex && dropTarget.zone === 'left'}
	{@const centerActive = dropTarget?.paneIndex === paneIndex && dropTarget.zone === 'center'}
	{@const rightActive = dropTarget?.paneIndex === paneIndex && dropTarget.zone === 'right'}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="pointer-events-none absolute inset-0 z-100 flex {fileDrag
			? 'justify-between'
			: 'bg-brand/6'}"
	>
		{#if stripGap === null}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class={dropZoneClass('left', fileDrag, leftActive)}
				onmouseenter={() => ondropenter(paneIndex, 'left')}
				onmouseleave={() => ondropleave(paneIndex, 'left')}
			>
				<span
					class="pointer-events-none rounded-xs border border-brand/50 bg-surface-1 px-2 py-[3px] text-xs font-medium text-accent-foreground transition-opacity {leftActive
						? 'opacity-100'
						: 'opacity-0'}">{m.pane_split_left()}</span
				>
			</div>
			{#if zoneMode === 'all'}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class={dropZoneClass('center', fileDrag, centerActive)}
					onmouseenter={() => ondropenter(paneIndex, 'center')}
					onmouseleave={() => ondropleave(paneIndex, 'center')}
				>
					<span
						class="pointer-events-none rounded-xs border border-brand/50 bg-surface-1 px-2 py-[3px] text-xs font-medium text-accent-foreground transition-opacity {centerActive
							? 'opacity-100'
							: 'opacity-0'}">{m.pane_move_here()}</span
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
						: 'opacity-0'}">{m.pane_split_right()}</span
				>
			</div>
		{/if}
	</div>
{/if}
