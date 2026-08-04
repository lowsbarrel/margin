<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import * as pdfjsLib from 'pdfjs-dist';

	interface Props {
		data: Uint8Array;
	}

	let { data }: Props = $props();

	const SCALE = 1.2;

	pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
		'pdfjs-dist/build/pdf.worker.mjs',
		import.meta.url
	).toString();

	/** The placeholder box Svelte lays out for a page before it is rasterised. */
	interface PageSlot {
		num: number;
		width: number;
		height: number;
	}

	let containerEl = $state<HTMLDivElement>(undefined!);
	let errorMessage = $state<string | null>(null);
	// Assigned once and never mutated in place, so `$state.raw` — a deep proxy
	// over one object per page of a long document buys nothing here.
	let pages = $state.raw<PageSlot[]>([]);
	// `visible[i]` flips true the first time page i scrolls into range. It is both
	// the "already rendered" record the old `renderedPages` Set kept and the thing
	// that puts the <canvas> in the DOM, so a long document never allocates every
	// backing store up front.
	let visible = $state<boolean[]>([]);

	let pdf: pdfjsLib.PDFDocumentProxy | null = null;
	let observer: IntersectionObserver | null = null;

	onMount(async () => {
		try {
			pdf = await pdfjsLib.getDocument({ data }).promise;
			const slots: PageSlot[] = [];
			for (let i = 1; i <= pdf.numPages; i++) {
				const page = await pdf.getPage(i);
				const viewport = page.getViewport({ scale: SCALE });
				slots.push({ num: i, width: viewport.width, height: viewport.height });
			}
			// The observer has to exist before the wrappers render, because each one
			// registers itself with it as it is created.
			observer = new IntersectionObserver(handleIntersection, {
				root: containerEl.closest('.pdf-viewer'),
				rootMargin: '200px'
			});
			visible = new Array(slots.length).fill(false);
			pages = slots;
		} catch (err) {
			console.error('Failed to load PDF:', err);
			errorMessage = `Failed to load PDF: ${err instanceof Error ? err.message : String(err)}`;
		}
	});

	onDestroy(() => {
		observer?.disconnect();
		observer = null;
		pdf?.destroy();
	});

	function handleIntersection(entries: IntersectionObserverEntry[]) {
		for (const entry of entries) {
			if (!entry.isIntersecting) continue;
			const pageNum = Number((entry.target as HTMLElement).dataset.pageNum);
			if (pageNum >= 1) visible[pageNum - 1] = true;
		}
	}

	/**
	 * Registers a page wrapper with the shared IntersectionObserver. An action, so
	 * the element is handed over the moment Svelte creates it and handed back when
	 * Svelte removes it — no querying into a tree the runtime owns.
	 */
	function watchVisibility(el: HTMLElement) {
		observer?.observe(el);
		return {
			destroy() {
				observer?.unobserve(el);
			}
		};
	}

	/**
	 * Rasterises a page into the <canvas> Svelte just created. PDF.js paints into
	 * the element's own 2D context and inserts nothing, so the whole subtree stays
	 * under Svelte's control. Sizing the canvas back down on teardown releases the
	 * backing store straight away rather than waiting on GC.
	 */
	function renderPage(canvas: HTMLCanvasElement, pageNum: number) {
		let cancelled = false;

		(async () => {
			if (!pdf) return;
			const page = await pdf.getPage(pageNum);
			if (cancelled) return;
			const viewport = page.getViewport({ scale: SCALE });
			canvas.width = viewport.width;
			canvas.height = viewport.height;
			await page.render({ canvas, viewport }).promise;
		})().catch((err) => {
			if (!cancelled) console.error(`Failed to render PDF page ${pageNum}:`, err);
		});

		return {
			destroy() {
				cancelled = true;
				canvas.width = 0;
				canvas.height = 0;
			}
		};
	}
</script>

<!--
	`pdf-viewer` is a load-bearing class name, not styling: the mount handler
	resolves the IntersectionObserver root with
	`containerEl.closest('.pdf-viewer')`. `pdf-pages`, `pdf-page-wrapper` and
	`pdf-page` name the scroller, the per-page box and the canvas PDF.js paints
	into, and stay as the stable handles for this view.

	The wrappers and canvases used to be built with document.createElement, which
	is why their chrome had to live in a `:global()` <style> block — Svelte owns
	them now, so it is plain utilities on the markup.
-->
<div class="pdf-viewer h-full w-full overflow-auto bg-surface-2">
	{#if errorMessage}
		<div
			class="flex h-full items-center justify-center p-6 text-center text-[13px] text-subtle-foreground"
		>
			{errorMessage}
		</div>
	{:else}
		<div class="pdf-pages flex flex-col items-center gap-2 p-4" bind:this={containerEl}>
			{#each pages as page (page.num)}
				<div
					class="pdf-page-wrapper max-w-full rounded-xs shadow-[var(--shadow-md)]"
					data-page-num={page.num}
					style:width="{page.width}px"
					style:height="{page.height}px"
					use:watchVisibility
				>
					{#if visible[page.num - 1]}
						<canvas class="pdf-page block h-auto max-w-full" use:renderPage={page.num}></canvas>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>
