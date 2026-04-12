<script lang="ts">
  import { X } from "lucide-svelte";
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    onclose: () => void;
    children: Snippet;
    width?: string;
  }

  let { title, onclose, children, width = "520px" }: Props = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="overlay" onclick={onclose}>
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="modal"
    style="width: {width}"
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-label={title}
  >
    <div class="modal-header">
      <h2>{title}</h2>
      <button class="close-btn" onclick={onclose} aria-label="Close">
        <X size={16} />
      </button>
    </div>
    <div class="modal-content">
      {@render children()}
    </div>
  </div>
</div>

<style>
  /* Full-screen overlay — just a backdrop, no scrolling here */
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 40px 16px;
    overflow: hidden;
  }

  .modal {
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    animation: slideUp 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .modal-header {
    position: sticky;
    top: 0;
    z-index: 2;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 20px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-primary);
    border-radius: var(--radius) var(--radius) 0 0;
  }

  .modal-header h2 {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
  }

  .close-btn {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    background: none;
    border: none;
    color: var(--text-muted);
    border-radius: var(--radius-xs);
    cursor: pointer;
  }
  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .modal-content {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 0 0 auto;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
