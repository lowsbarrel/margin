<script lang="ts">
	import { onMount, onDestroy, tick } from 'svelte';
	import * as pdfjsLib from 'pdfjs-dist';
	import {
		ChevronDown,
		ChevronLeft,
		ChevronRight,
		ChevronUp,
		Expand,
		ExternalLink,
		Search,
		X,
		ZoomIn,
		ZoomOut
	} from '@lucide/svelte';
	import { openPath } from '@tauri-apps/plugin-opener';
	import { toOsPath } from '$lib/editor/image-url';
	import { toast } from '$lib/stores/toast.svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		data: Uint8Array;
		/** Vault-absolute path, for handing the file to the OS. */
		path: string;
		name: string;
	}

	let { data, path, name }: Props = $props();

	const MIN_ZOOM = 0.25;
	const MAX_ZOOM = 6;
	const ZOOM_STEP = 1.25;
	/** Horizontal padding of the page column: px-4 on both sides. */
	const PAGE_PADDING = 32;
	const CONTROL =
		'flex h-6.5 min-w-6.5 items-center justify-center rounded-xs bg-transparent px-1.5 text-sm text-subtle-foreground transition-colors hover:bg-surface-3 hover:text-foreground disabled:opacity-40 disabled:hover:bg-transparent';

	pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
		'pdfjs-dist/build/pdf.worker.mjs',
		import.meta.url
	).toString();

	/** The placeholder box Svelte lays out for a page before it is rasterised. */
	interface PageSlot {
		num: number;
		/** Page size in PDF units; page 1's until the page itself is measured. */
		width: number;
		height: number;
	}

	interface PageText {
		text: string;
		/** Offset of each text item's first character within `text`. */
		starts: number[];
	}

	interface Match {
		page: number;
		start: number;
		end: number;
	}

	let containerEl = $state<HTMLDivElement>(undefined!);
	let errorMessage = $state<string | null>(null);
	// Assigned once and never mutated in place, so `$state.raw` — a deep proxy
	// over one object per page of a long document buys nothing here.
	let slots = $state.raw<PageSlot[]>([]);
	/** Pages whose canvas and text layer are in the DOM right now. */
	let mounted = $state.raw<boolean[]>([]);
	let baseWidth = $state(0);
	let containerWidth = $state(0);
	let zoom = $state(1);
	let fitWidth = $state(true);
	let currentPage = $state(1);
	let pageInput = $state('1');
	let pageEditing = $state(false);
	let findOpen = $state(false);
	let query = $state('');
	let matches = $state.raw<Match[]>([]);
	let matchIndex = $state(0);
	/** Page currently carrying a find highlight, so it can be cleared first. */
	let markedPage = $state<number | null>(null);
	let findInput = $state<HTMLInputElement>(undefined!);
	let searching = $state(false);

	let pdf: pdfjsLib.PDFDocumentProxy | null = null;
	// v6 moved destroy off the document proxy onto the loading task that owns it.
	let loadingTask: pdfjsLib.PDFDocumentLoadingTask | null = null;
	let observer: IntersectionObserver | null = null;
	let pageObserver: IntersectionObserver | null = null;
	let searchToken = 0;
	// Find needs the rendered text layers and the text it searched, but neither
	// may be reactive: a cache write must not re-render the viewer's canvases.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const layers = new Map<number, pdfjsLib.TextLayer>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const pageTexts = new Map<number, PageText>();

	let fitted = $derived(
		baseWidth > 0 && containerWidth > 0 ? (containerWidth - PAGE_PADDING) / baseWidth : 1
	);
	let scale = $derived(Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, fitWidth ? fitted : zoom)));
	let numPages = $derived(slots.length);

	onMount(async () => {
		try {
			loadingTask = pdfjsLib.getDocument({ data });
			pdf = await loadingTask.promise;
			// One measurement sizes every placeholder: `getPage` on all pages up
			// front is what made a long document's first paint slow, and the rest
			// of the document only needs a box of the right shape to scroll
			// through — each page corrects its own box when it is rendered.
			const first = await pdf.getPage(1);
			const viewport = first.getViewport({ scale: 1 });
			baseWidth = viewport.width;
			// The observers have to exist before the wrappers render, because each
			// one registers itself with them as it is created.
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
		searchToken++;
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

	$effect(() => {
		if (!findOpen) return;
		// Runs after the bar is in the DOM, so Cmd/Ctrl+F always lands the caret
		// in the field; a ref grabbed inside the key handler can outlive its node.
		findInput?.focus();
		findInput?.select();
	});

	$effect(() => {
		if (!pageEditing) pageInput = String(currentPage);
	});

	/**
	 * Registers a page wrapper with both observers. An action, so the element is
	 * handed over the moment Svelte creates it and handed back when Svelte
	 * removes it — no querying into a tree the runtime owns.
	 */
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

	/** Mounts a page near the viewport, unmounts it — with its canvas — once it is far. */
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

	function commitPage() {
		const parsed = Number.parseInt(pageInput, 10);
		if (Number.isFinite(parsed)) scrollToPage(parsed);
	}

	/** Rasterises a page into the <canvas> Svelte just created, at device resolution. */
	function renderPage(canvas: HTMLCanvasElement, slot: PageSlot) {
		let cancelled = false;

		(async () => {
			if (!pdf) return;
			const page = await pdf.getPage(slot.num);
			if (cancelled) return;
			const viewport = page.getViewport({ scale });
			const ratio = window.devicePixelRatio || 1;
			canvas.width = Math.floor(viewport.width * ratio);
			canvas.height = Math.floor(viewport.height * ratio);
			canvas.style.width = `${viewport.width}px`;
			canvas.style.height = `${viewport.height}px`;
			await page.render({
				canvas,
				viewport,
				transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0]
			}).promise;
			// A page whose real size differs from page 1's placeholder corrects its
			// own box; only the page just rendered is remeasured.
			const unscaled = page.getViewport({ scale: 1 });
			if (
				!cancelled &&
				(Math.abs(slot.width - unscaled.width) > 0.5 ||
					Math.abs(slot.height - unscaled.height) > 0.5)
			) {
				slots = slots.map((s) =>
					s.num === slot.num ? { ...s, width: unscaled.width, height: unscaled.height } : s
				);
			}
		})().catch((err) => {
			if (!cancelled) console.error(`Failed to render PDF page ${slot.num}:`, err);
		});

		return {
			destroy() {
				cancelled = true;
				canvas.width = 0;
				canvas.height = 0;
			}
		};
	}

	/** Builds the selectable text layer over a page's canvas. */
	function textLayer(el: HTMLDivElement, pageNum: number) {
		let cancelled = false;
		let layer: pdfjsLib.TextLayer | null = null;

		(async () => {
			if (!pdf) return;
			const page = await pdf.getPage(pageNum);
			if (cancelled) return;
			const viewport = page.getViewport({ scale });
			const content = await page.getTextContent();
			if (cancelled) return;
			layer = new pdfjsLib.TextLayer({
				textContentSource: content,
				container: el,
				viewport
			});
			await layer.render();
			if (cancelled) {
				layer.cancel();
				return;
			}
			layers.set(pageNum, layer);
			// A page that scrolls back into view repaints without its highlight, so
			// the current match is re-marked as soon as the layer exists again.
			const current = matches[matchIndex];
			if (current?.page === pageNum) markMatch(pageNum, current);
		})().catch((err) => {
			if (!cancelled) console.error(`Failed to lay out PDF text for page ${pageNum}:`, err);
		});

		return {
			destroy() {
				cancelled = true;
				layers.delete(pageNum);
				layer?.cancel();
				el.replaceChildren();
			}
		};
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

	function pageText(pageNum: number): Promise<PageText> {
		const cached = pageTexts.get(pageNum);
		if (cached) return Promise.resolve(cached);
		return pdf!.getPage(pageNum).then((page) =>
			page.getTextContent().then((content) => {
				const starts: number[] = [];
				let text = '';
				for (const item of content.items) {
					if (!('str' in item)) continue;
					starts.push(text.length);
					text += item.str;
				}
				const entry = { text, starts };
				pageTexts.set(pageNum, entry);
				return entry;
			})
		);
	}

	/**
	 * Scans every page for the query, publishing matches as each page is read so
	 * the count fills in instead of the bar sitting empty on a long document.
	 */
	async function runSearch() {
		const needle = query.trim().toLowerCase();
		const token = ++searchToken;
		matchIndex = 0;
		if (!needle || !pdf) {
			matches = [];
			searching = false;
			return;
		}
		searching = true;
		const found: Match[] = [];
		for (let page = 1; page <= pdf.numPages; page++) {
			let text: PageText;
			try {
				text = await pageText(page);
			} catch {
				continue;
			}
			if (token !== searchToken) return;
			const haystack = text.text.toLowerCase();
			let at = haystack.indexOf(needle);
			while (at >= 0) {
				found.push({ page, start: at, end: at + needle.length });
				at = haystack.indexOf(needle, at + needle.length);
			}
			matches = [...found];
		}
		searching = false;
		if (found.length > 0) await gotoMatch(0);
	}

	/** Waits for the text layer of a page that is only now scrolling into view. */
	async function awaitLayer(pageNum: number): Promise<pdfjsLib.TextLayer | null> {
		for (let attempt = 0; attempt < 40; attempt++) {
			const layer = layers.get(pageNum);
			if (layer) return layer;
			await new Promise((resolve) => setTimeout(resolve, 25));
		}
		return null;
	}

	async function gotoMatch(index: number) {
		if (matches.length === 0) return;
		const at = ((index % matches.length) + matches.length) % matches.length;
		if (markedPage !== null) clearMarks(markedPage);
		matchIndex = at;
		markedPage = null;
		const match = matches[at];
		scrollToPage(match.page);
		await tick();
		await awaitLayer(match.page);
		markMatch(match.page, match);
	}

	/** Unwraps the find highlights of a page, merging their text back together. */
	function clearMarks(pageNum: number) {
		const layer = layers.get(pageNum);
		if (!layer) return;
		for (const div of layer.textDivs) {
			const marks = Array.from(div.querySelectorAll('.pdf-find-hit'));
			if (marks.length === 0) continue;
			for (const mark of marks) mark.replaceWith(...Array.from(mark.childNodes));
			div.normalize();
		}
	}

	/**
	 * Wraps one match in its span(s). A highlight rather than a document
	 * selection: selecting text outside a focused input is what stops Chrome
	 * accepting the next keystroke into the find bar.
	 */
	function markMatch(pageNum: number, match: Match) {
		const layer = layers.get(pageNum);
		const text = pageTexts.get(pageNum);
		if (!layer || !text) return;
		// The layer's spans follow the text items, so an item's offset in the
		// concatenated text is the index of the span that carries it.
		let startSpan = 0;
		while (startSpan + 1 < text.starts.length && text.starts[startSpan + 1] <= match.start) {
			startSpan++;
		}
		let endSpan = startSpan;
		while (endSpan + 1 < text.starts.length && text.starts[endSpan + 1] < match.end) endSpan++;
		const from = layer.textDivs[startSpan];
		const to = layer.textDivs[endSpan];
		if (!from?.firstChild || !to?.firstChild) return;

		const range = document.createRange();
		range.setStart(
			from.firstChild,
			Math.min(match.start - text.starts[startSpan], from.textContent?.length ?? 0)
		);
		range.setEnd(
			to.firstChild,
			Math.min(match.end - text.starts[endSpan], to.textContent?.length ?? 0)
		);
		const mark = document.createElement('span');
		mark.className = 'pdf-find-hit';
		range.surroundContents(mark);
		markedPage = pageNum;
		mark.scrollIntoView({ block: 'center' });
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
		if (e.key === 'Enter' && findOpen && matches.length > 0) {
			e.preventDefault();
			void gotoMatch(matchIndex + (e.shiftKey ? -1 : 1));
		}
	}
</script>

<!--
	`pdf-viewer` names the view, `pdf-scroll` its scroller, `pdf-pages` the page
	column, `pdf-page-wrapper` a page's box and `pdf-page` the canvas PDF.js
	paints into. Those stay the stable handles for this view.
-->
<div class="pdf-viewer flex h-full w-full flex-col overflow-hidden bg-surface-2">
	{#if errorMessage}
		<div
			class="flex flex-1 items-center justify-center p-6 text-center text-[13px] text-subtle-foreground"
		>
			{errorMessage}
		</div>
	{:else}
		<!-- Focusable so Cmd/Ctrl+F and Escape reach the document's own find bar. -->
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
				<div
					class="absolute top-2 right-3 z-10 flex items-center gap-1 rounded-sm border border-border bg-background p-1 shadow-(--shadow-md)"
				>
					<Search size={13} class="ml-1 shrink-0 text-subtle-foreground" />
					<input
						bind:this={findInput}
						bind:value={query}
						oninput={runSearch}
						placeholder={m.pdf_find_placeholder()}
						aria-label={m.pdf_find_placeholder()}
						class="w-40 bg-transparent text-xs text-foreground outline-none placeholder:text-subtle-foreground"
					/>
					<span class="min-w-16 text-right text-xs text-subtle-foreground tabular-nums">
						{matches.length === 0
							? query.trim()
								? searching
									? '…'
									: m.pdf_find_none()
								: ''
							: `${matchIndex + 1} / ${matches.length}`}
					</span>
					<button
						class={CONTROL}
						onclick={() => gotoMatch(matchIndex - 1)}
						disabled={matches.length === 0}
						aria-label={m.pdf_find_previous()}
					>
						<ChevronUp size={14} />
					</button>
					<button
						class={CONTROL}
						onclick={() => gotoMatch(matchIndex + 1)}
						disabled={matches.length === 0}
						aria-label={m.pdf_find_next()}
					>
						<ChevronDown size={14} />
					</button>
					<button class={CONTROL} onclick={() => (findOpen = false)} aria-label={m.tab_close()}>
						<X size={14} />
					</button>
				</div>
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
									<canvas class="pdf-page block" use:renderPage={slot}></canvas>
									<div class="pdf-text-layer" use:textLayer={slot.num}></div>
								{/key}
							{/if}
						</div>
					{/each}
				</div>
			</div>
		</div>

		<div class="flex items-center gap-1 border-t border-border bg-surface-1 p-1.5">
			<span class="min-w-0 flex-1 truncate px-2 text-xs text-foreground" title={name}>{name}</span>
			<button
				class={CONTROL}
				onclick={() => zoomTo(scale / ZOOM_STEP)}
				aria-label={m.viewer_zoom_out()}
			>
				<ZoomOut size={14} />
			</button>
			<span class="min-w-12 text-center text-sm tabular-nums">{Math.round(scale * 100)}%</span>
			<button
				class={CONTROL}
				onclick={() => zoomTo(scale * ZOOM_STEP)}
				aria-label={m.viewer_zoom_in()}
			>
				<ZoomIn size={14} />
			</button>
			<button
				class={CONTROL}
				onclick={() => (fitWidth = true)}
				title={m.pdf_fit_width()}
				aria-label={m.pdf_fit_width()}
				aria-pressed={fitWidth}
			>
				<Expand size={14} />
			</button>
			<button
				class={CONTROL}
				onclick={openExternally}
				title={m.viewer_open_external()}
				aria-label={m.viewer_open_external()}
			>
				<ExternalLink size={14} />
			</button>

			<span class="mx-1 h-4 w-px shrink-0 bg-border"></span>

			<button
				class={CONTROL}
				onclick={() => scrollToPage(currentPage - 1)}
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
				class={CONTROL}
				onclick={() => scrollToPage(currentPage + 1)}
				disabled={currentPage >= numPages}
				aria-label={m.viewer_next()}
			>
				<ChevronRight size={14} />
			</button>
		</div>
	{/if}
</div>

<style>
	/* pdf.js lays the text layer out in absolute coordinates and reads the scale
	   from `--total-scale-factor`, so the rules it expects are restated here:
	   transparent, non-wrapping spans over the canvas, selectable. */
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
		/* app.css turns selection off for the whole app chrome; the document's
		   text — and the find bar's selection of it — has to opt back in. */
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
	/* The find bar's hit: a highlight (not a selection) so the match stays
	   marked while the user keeps typing in the field. Global because the span
	   is created by the range, so it never carries Svelte's scope class. */
	.pdf-text-layer :global(.pdf-find-hit) {
		background: var(--color-highlight-yellow);
		box-shadow: 0 0 0 1px var(--color-brand-24);
		border-radius: 2px;
	}
</style>
