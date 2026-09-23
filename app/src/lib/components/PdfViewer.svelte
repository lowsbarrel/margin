<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import * as pdfjsLib from 'pdfjs-dist';
	import { openPath } from '@tauri-apps/plugin-opener';
	import { toOsPath } from '$lib/editor/image-url';
	import { toast } from '$lib/stores/toast.svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { createPageActions, type PageSlot } from './pdf/page-render';
	import { PdfSearch } from './pdf/pdf-search.svelte';
	import PdfFindBar from './pdf/PdfFindBar.svelte';
	import PdfToolbar from './pdf/PdfToolbar.svelte';

	interface Props {
		data: Uint8Array;
		path: string;
		name: string;
	}

	let { data, path, name }: Props = $props();

	const MIN_ZOOM = 0.25;
	const MAX_ZOOM = 6;
	const ZOOM_STEP = 1.25;
	const PAGE_PADDING = 32;

	pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
		'pdfjs-dist/build/pdf.worker.mjs',
		import.meta.url
	).toString();

	let containerEl = $state<HTMLDivElement>(undefined!);
	let errorMessage = $state<string | null>(null);
	let slots = $state.raw<PageSlot[]>([]);
	let mounted = $state.raw<boolean[]>([]);
	let baseWidth = $state(0);
	let containerWidth = $state(0);
	let zoom = $state(1);
	let fitWidth = $state(true);
	let currentPage = $state(1);
	let findOpen = $state(false);

	let pdf: pdfjsLib.PDFDocumentProxy | null = null;
	let loadingTask: pdfjsLib.PDFDocumentLoadingTask | null = null;
	let observer: IntersectionObserver | null = null;
	let pageObserver: IntersectionObserver | null = null;

	let fitted = $derived(
		baseWidth > 0 && containerWidth > 0 ? (containerWidth - PAGE_PADDING) / baseWidth : 1
	);
	let scale = $derived(Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, fitWidth ? fitted : zoom)));
	let numPages = $derived(slots.length);

	const search = new PdfSearch({
		document: () => pdf,
		scrollToPage: (page) => scrollToPage(page)
	});

	const pages = createPageActions({
		document: () => pdf,
		scale: () => scale,
		onMeasured: (num, width, height) => {
			slots = slots.map((slot) => (slot.num === num ? { ...slot, width, height } : slot));
		},
		onLayerReady: (num, layer) => search.registerLayer(num, layer),
		onLayerGone: (num) => search.forgetLayer(num)
	});

	onMount(async () => {
		try {
			loadingTask = pdfjsLib.getDocument({ data });
			pdf = await loadingTask.promise;
			const first = await pdf.getPage(1);
			const viewport = first.getViewport({ scale: 1 });
			baseWidth = viewport.width;
			// The observers must exist first: each page wrapper registers itself as it renders.
			observer = new IntersectionObserver(handleVisibility, {
				root: containerEl,
				rootMargin: '500px'
			});
			pageObserver = new IntersectionObserver(handleCurrentPage, {
				root: containerEl,
				rootMargin: '-45% 0px -45% 0px'
			});
			slots = Array.from({ length: pdf.numPages }, (_, i) => ({
				num: i + 1,
				width: viewport.width,
				height: viewport.height
			}));
			mounted = new Array(pdf.numPages).fill(false);
		} catch (err) {
			console.error('Failed to load PDF:', err);
			errorMessage = m.pdf_load_failed({
				error: err instanceof Error ? err.message : String(err)
			});
		}
	});

	onDestroy(() => {
		observer?.disconnect();
		pageObserver?.disconnect();
		observer = null;
		pageObserver = null;
		search.cancel();
		void loadingTask?.destroy();
	});

	$effect(() => {
		const el = containerEl;
		if (!el) return;
		const resizeObserver = new ResizeObserver(([entry]) => {
			containerWidth = entry.contentRect.width;
		});
		resizeObserver.observe(el);
		return () => resizeObserver.disconnect();
	});

	function watchVisibility(el: HTMLElement) {
		observer?.observe(el);
		pageObserver?.observe(el);
		return {
			destroy() {
				observer?.unobserve(el);
				pageObserver?.unobserve(el);
			}
		};
	}

	function handleVisibility(entries: IntersectionObserverEntry[]) {
		let next: boolean[] | null = null;
		for (const entry of entries) {
			const pageNum = Number((entry.target as HTMLElement).dataset.pageNum);
			if (pageNum < 1 || pageNum > mounted.length) continue;
			if (mounted[pageNum - 1] === entry.isIntersecting) continue;
			next ??= [...mounted];
			next[pageNum - 1] = entry.isIntersecting;
		}
		if (next) mounted = next;
	}

	function handleCurrentPage(entries: IntersectionObserverEntry[]) {
		for (const entry of entries) {
			if (!entry.isIntersecting) continue;
			const pageNum = Number((entry.target as HTMLElement).dataset.pageNum);
			if (pageNum >= 1) currentPage = pageNum;
		}
	}

	function scrollToPage(page: number) {
		const target = Math.min(numPages, Math.max(1, Math.round(page)));
		if (!Number.isFinite(target) || numPages === 0) return;
		currentPage = target;
		const el = containerEl?.querySelector(`[data-page-num="${target}"]`);
		el?.scrollIntoView({ block: 'start' });
	}

	async function openExternally() {
		try {
			await openPath(toOsPath(path));
		} catch (err) {
			toast.error(m.toast_cannot_open_file({ error: String(err) }));
		}
	}

	function zoomTo(next: number) {
		fitWidth = false;
		zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, next));
	}

	function handleKeydown(e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
			e.preventDefault();
			findOpen = true;
			return;
		}
		if (e.key === 'Escape' && findOpen) {
			findOpen = false;
			return;
		}
		if (e.key === 'Enter' && findOpen && search.matches.length > 0) {
			e.preventDefault();
			void search.goTo(search.matchIndex + (e.shiftKey ? -1 : 1));
		}
	}
</script>

<div class="pdf-viewer flex h-full w-full flex-col overflow-hidden bg-surface-2">
	{#if errorMessage}
		<div
			class="flex flex-1 items-center justify-center p-6 text-center text-[13px] text-subtle-foreground"
		>
			{errorMessage}
		</div>
	{:else}
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<div
			class="relative min-h-0 flex-1"
			bind:this={containerEl}
			tabindex={0}
			role="application"
			aria-label={name}
			onkeydown={handleKeydown}
		>
			{#if findOpen}
				<PdfFindBar {search} onclose={() => (findOpen = false)} />
			{/if}

			<div class="pdf-scroll h-full [scrollbar-gutter:stable] overflow-auto">
				<div class="pdf-pages flex flex-col items-center gap-3 px-4 py-4">
					{#each slots as slot (slot.num)}
						<div
							class="pdf-page-wrapper relative max-w-full rounded-xs shadow-(--shadow-md)"
							data-page-num={slot.num}
							style:width="{slot.width * scale}px"
							style:height="{slot.height * scale}px"
							style:--total-scale-factor={scale}
							style:--scale-round-x="1px"
							style:--scale-round-y="1px"
							use:watchVisibility
						>
							{#if mounted[slot.num - 1]}
								{#key scale}
									<canvas class="pdf-page block" use:pages.renderPage={slot}></canvas>
									<div class="pdf-text-layer" use:pages.textLayer={slot.num}></div>
								{/key}
							{/if}
						</div>
					{/each}
				</div>
			</div>
		</div>

		<PdfToolbar
			{name}
			{scale}
			{fitWidth}
			{currentPage}
			{numPages}
			onzoomout={() => zoomTo(scale / ZOOM_STEP)}
			onzoomin={() => zoomTo(scale * ZOOM_STEP)}
			onfit={() => (fitWidth = true)}
			onexternal={openExternally}
			onpage={scrollToPage}
		/>
	{/if}
</div>

<style>
	/* PDF.js positions the text layer absolutely and reads --total-scale-factor. */
	.pdf-text-layer {
		position: absolute;
		inset: 0;
		overflow: clip;
		line-height: 1;
		text-align: initial;
		text-size-adjust: none;
		forced-color-adjust: none;
		transform-origin: 0 0;
		z-index: 1;
		-webkit-user-select: text;
		user-select: text;
	}
	.pdf-text-layer :global(:is(span, br)) {
		position: absolute;
		color: transparent;
		white-space: pre;
		cursor: text;
		transform-origin: 0% 0%;
	}
	.pdf-text-layer :global(> :not(.markedContent)) {
		font-size: calc(var(--total-scale-factor) * var(--font-height));
		transform: rotate(var(--rotate, 0deg)) scaleX(var(--scale-x, 1));
	}
	.pdf-text-layer :global(.markedContent) {
		display: contents;
	}
	.pdf-text-layer :global(::selection) {
		background: color-mix(in srgb, AccentColor, transparent 70%);
	}
	.pdf-text-layer :global(.pdf-find-hit) {
		background: var(--color-highlight-yellow);
		box-shadow: 0 0 0 1px var(--color-brand-24);
		border-radius: 2px;
	}
</style>
