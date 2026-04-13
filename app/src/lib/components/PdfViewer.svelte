<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import * as pdfjsLib from "pdfjs-dist";

  interface Props {
    /** Raw PDF bytes */
    data: Uint8Array;
  }

  let { data }: Props = $props();
  let containerEl = $state<HTMLDivElement>(undefined!);
  let pageCount = $state(0);
  let scale = $state(1.2);
  let errorMessage = $state<string | null>(null);

  pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
    "pdfjs-dist/build/pdf.worker.mjs",
    import.meta.url,
  ).toString();

  let pdf: pdfjsLib.PDFDocumentProxy | null = null;
  let canvases: HTMLCanvasElement[] = [];
  let observer: IntersectionObserver | null = null;
  let renderedPages = new Set<number>();

  onMount(async () => {
    try {
      pdf = await pdfjsLib.getDocument({ data }).promise;
      pageCount = pdf.numPages;
      await setupPages();
    } catch (err) {
      console.error("Failed to load PDF:", err);
      errorMessage = `Failed to load PDF: ${err instanceof Error ? err.message : String(err)}`;
    }
  });

  onDestroy(() => {
    observer?.disconnect();
    observer = null;
    cleanupCanvases();
    pdf?.destroy();
  });

  function cleanupCanvases() {
    for (const canvas of canvases) {
      // Release GPU memory by zeroing dimensions before removing
      canvas.width = 0;
      canvas.height = 0;
    }
    canvases = [];
  }

  async function setupPages() {
    if (!pdf || !containerEl) return;
    cleanupCanvases();
    containerEl.innerHTML = "";
    renderedPages.clear();

    // Create placeholders with correct dimensions
    for (let i = 1; i <= pdf.numPages; i++) {
      const page = await pdf.getPage(i);
      const viewport = page.getViewport({ scale });

      const wrapper = document.createElement("div");
      wrapper.classList.add("pdf-page-wrapper");
      wrapper.style.width = `${viewport.width}px`;
      wrapper.style.height = `${viewport.height}px`;
      wrapper.dataset.pageNum = String(i);
      containerEl.appendChild(wrapper);
    }

    // Observe visibility
    observer = new IntersectionObserver(handleIntersection, {
      root: containerEl.closest(".pdf-viewer"),
      rootMargin: "200px",
    });

    containerEl.querySelectorAll(".pdf-page-wrapper").forEach((el) => observer!.observe(el));
  }

  async function handleIntersection(entries: IntersectionObserverEntry[]) {
    for (const entry of entries) {
      const pageNum = Number((entry.target as HTMLElement).dataset.pageNum);
      if (entry.isIntersecting && !renderedPages.has(pageNum)) {
        renderedPages.add(pageNum);
        await renderPage(pageNum, entry.target as HTMLElement);
      }
    }
  }

  async function renderPage(pageNum: number, wrapper: HTMLElement) {
    if (!pdf) return;
    const page = await pdf.getPage(pageNum);
    const viewport = page.getViewport({ scale });
    const canvas = document.createElement("canvas");
    canvas.width = viewport.width;
    canvas.height = viewport.height;
    canvas.classList.add("pdf-page");
    const ctx = canvas.getContext("2d")!;
    await page.render({ canvasContext: ctx, viewport, canvas } as any).promise;
    wrapper.innerHTML = "";
    wrapper.appendChild(canvas);
    canvases.push(canvas);
  }
</script>

<div class="pdf-viewer">
  {#if errorMessage}
    <div class="pdf-error">{errorMessage}</div>
  {:else}
    <div class="pdf-pages" bind:this={containerEl}></div>
  {/if}
</div>

<style>
  .pdf-viewer {
    width: 100%;
    height: 100%;
    overflow: auto;
    background: var(--bg-tertiary);
  }

  .pdf-pages {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: var(--space-lg);
  }

  .pdf-pages :global(.pdf-page-wrapper) {
    max-width: 100%;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    border-radius: 2px;
  }

  .pdf-pages :global(.pdf-page) {
    display: block;
    max-width: 100%;
    height: auto;
  }

  .pdf-error {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--text-muted);
    font-size: 13px;
    padding: var(--space-xl);
    text-align: center;
  }
</style>
