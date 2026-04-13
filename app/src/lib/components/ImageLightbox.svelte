<script lang="ts">
  interface Props {
    src: string;
    alt?: string;
    onclose: () => void;
  }

  let { src, alt = "Image", onclose }: Props = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="lightbox-overlay"
  onclick={onclose}
  onkeydown={handleKeydown}
>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <img
    {src}
    {alt}
    class="lightbox-img"
    onclick={(e) => e.stopPropagation()}
  />
  <button
    class="lightbox-close"
    onclick={onclose}
    aria-label="Close">&times;</button
  >
</div>

<style>
  .lightbox-overlay {
    position: fixed;
    inset: 0;
    z-index: 300;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.85);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    cursor: zoom-out;
  }

  .lightbox-img {
    max-width: 90vw;
    max-height: 90vh;
    object-fit: contain;
    border-radius: 6px;
    cursor: default;
    user-select: none;
    box-shadow: 0 8px 40px rgba(0, 0, 0, 0.5);
  }

  .lightbox-close {
    position: absolute;
    top: 16px;
    right: 20px;
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.7);
    font-size: 2rem;
    line-height: 1;
    cursor: pointer;
    padding: 4px 10px;
    border-radius: 6px;
    transition:
      color 0.12s,
      background 0.12s;
  }

  .lightbox-close:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.1);
  }
</style>
