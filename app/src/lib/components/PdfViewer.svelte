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

  onMount(async () => {
    try {
      pdf = await pdfjsLib.getDocument({ data }).promise;
      pageCount = pdf.numPages;
      renderAllPages();
    } catch (err) {
      console.error("Failed to load PDF:", err);
      errorMessage = `Failed to load PDF: ${err instanceof Error ? err.message : String(err)}`;
    }
  });

  onDestroy(() => {
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

  async function renderAllPages() {
    if (!pdf || !containerEl) return;
    cleanupCanvases();
    containerEl.innerHTML = "";

    for (let i = 1; i <= pdf.numPages; i++) {
      const page = await pdf.getPage(i);
      const viewport = page.getViewport({ scale });

      const canvas = document.createElement("canvas");
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      canvas.classList.add("pdf-page");

      const ctx = canvas.getContext("2d")!;
      await page.render({ canvasContext: ctx, viewport, canvas }).promise;

      canvases.push(canvas);
      containerEl.appendChild(canvas);
    }
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

  .pdf-pages :global(.pdf-page) {
    max-width: 100%;
    height: auto;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    border-radius: 2px;
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
