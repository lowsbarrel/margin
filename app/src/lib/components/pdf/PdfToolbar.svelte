<script lang="ts">
	import { ChevronLeft, ChevronRight, Expand, ExternalLink, ZoomIn, ZoomOut } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { VIEWER_CONTROL } from './control-classes';

	interface Props {
		name: string;
		scale: number;
		fitWidth: boolean;
		currentPage: number;
		numPages: number;
		onzoomin: () => void;
		onzoomout: () => void;
		onfit: () => void;
		onexternal: () => void;
		onpage: (page: number) => void;
	}

	let {
		name,
		scale,
		fitWidth,
		currentPage,
		numPages,
		onzoomin,
		onzoomout,
		onfit,
		onexternal,
		onpage
	}: Props = $props();

	let pageInput = $state('1');
	let pageEditing = $state(false);

	$effect(() => {
		if (!pageEditing) pageInput = String(currentPage);
	});

	function commitPage() {
		const parsed = Number.parseInt(pageInput, 10);
		if (Number.isFinite(parsed)) onpage(parsed);
	}
</script>

<div class="flex items-center gap-1 border-t border-border bg-surface-1 p-1.5">
	<span class="min-w-0 flex-1 truncate px-2 text-xs text-foreground" title={name}>{name}</span>
	<button class={VIEWER_CONTROL} onclick={onzoomout} aria-label={m.viewer_zoom_out()}>
		<ZoomOut size={14} />
	</button>
	<span class="min-w-12 text-center text-sm tabular-nums">{Math.round(scale * 100)}%</span>
	<button class={VIEWER_CONTROL} onclick={onzoomin} aria-label={m.viewer_zoom_in()}>
		<ZoomIn size={14} />
	</button>
	<button
		class={VIEWER_CONTROL}
		onclick={onfit}
		title={m.pdf_fit_width()}
		aria-label={m.pdf_fit_width()}
		aria-pressed={fitWidth}
	>
		<Expand size={14} />
	</button>
	<button
		class={VIEWER_CONTROL}
		onclick={onexternal}
		title={m.viewer_open_external()}
		aria-label={m.viewer_open_external()}
	>
		<ExternalLink size={14} />
	</button>

	<span class="mx-1 h-4 w-px shrink-0 bg-border"></span>

	<button
		class={VIEWER_CONTROL}
		onclick={() => onpage(currentPage - 1)}
		disabled={currentPage <= 1}
		aria-label={m.viewer_previous()}
	>
		<ChevronLeft size={14} />
	</button>
	<input
		bind:value={pageInput}
		onfocus={() => (pageEditing = true)}
		onblur={() => {
			pageEditing = false;
			commitPage();
		}}
		onkeydown={(e) => {
			if (e.key === 'Enter') {
				e.preventDefault();
				pageEditing = false;
				commitPage();
			}
		}}
		aria-label={m.pdf_page_number()}
		class="h-6.5 w-11 rounded-xs border border-border bg-background text-center text-xs text-foreground tabular-nums focus:border-ring focus:outline-none"
	/>
	<span class="px-1 text-xs text-subtle-foreground tabular-nums">/ {numPages}</span>
	<button
		class={VIEWER_CONTROL}
		onclick={() => onpage(currentPage + 1)}
		disabled={currentPage >= numPages}
		aria-label={m.viewer_next()}
	>
		<ChevronRight size={14} />
	</button>
</div>
