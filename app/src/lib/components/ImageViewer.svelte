<script lang="ts">
  interface Props {
    src: string;
    alt: string;
  }

  let { src, alt }: Props = $props();
  let scale = $state(1);
  const ZOOM_STEP = 0.15;
  const MIN_SCALE = 0.1;
  const MAX_SCALE = 5;

  function handleWheel(e: WheelEvent) {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    const delta = e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale + delta));
  }

  function resetZoom() {
    scale = 1;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="image-viewer" onwheel={handleWheel}>
  <div class="image-container">
    <img {src} {alt} style:transform="scale({scale})" draggable="false" />
  </div>
  <div class="zoom-bar">
    <button
      class="zoom-btn"
      onclick={() => (scale = Math.max(MIN_SCALE, scale - ZOOM_STEP))}>−</button
    >
    <button class="zoom-label" onclick={resetZoom}
      >{Math.round(scale * 100)}%</button
    >
    <button
      class="zoom-btn"
      onclick={() => (scale = Math.min(MAX_SCALE, scale + ZOOM_STEP))}>+</button
    >
  </div>
</div>

<style>
  .image-viewer {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .image-container {
    flex: 1;
    overflow: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-xl);
  }

  img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    transition: transform 0.1s ease;
    border-radius: var(--radius-xs);
    user-select: none;
  }

  .zoom-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
    padding: 6px;
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-secondary);
  }

  .zoom-btn,
  .zoom-label {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 0.8rem;
    padding: 2px 8px;
    border-radius: var(--radius-xs);
    cursor: pointer;
  }

  .zoom-btn:hover,
  .zoom-label:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .zoom-label {
    min-width: 48px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
</style>
