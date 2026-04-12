<script lang="ts">
  import { toast } from "$lib/stores/toast.svelte";
  import { Check, AlertCircle, Info, X } from "lucide-svelte";
</script>

{#if toast.items.length > 0}
  <div class="toast-container">
    {#each toast.items as item (item.id)}
      <div class="toast {item.type}" role="alert">
        <span class="toast-icon">
          {#if item.type === "success"}
            <Check size={14} />
          {:else if item.type === "error"}
            <AlertCircle size={14} />
          {:else}
            <Info size={14} />
          {/if}
        </span>
        <span class="toast-msg">{item.message}</span>
        {#if item.action}
          <button class="toast-action" onclick={() => { item.action!.onClick(); toast.dismiss(item.id); }}>
            {item.action.label}
          </button>
        {/if}
        <button class="toast-close" onclick={() => toast.dismiss(item.id)}>
          <X size={12} />
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-container {
    position: fixed;
    bottom: 40px;
    right: 16px;
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
    z-index: 200;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: 10px 14px;
    background: var(--glass-bg);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-sm);
    box-shadow: var(--glass-shadow);
    font-size: 0.8rem;
    color: var(--text-primary);
    pointer-events: auto;
    animation: toastIn 0.25s cubic-bezier(0.16, 1, 0.3, 1);
    max-width: 360px;
  }

  .toast.success .toast-icon {
    color: var(--success);
  }
  .toast.error .toast-icon {
    color: var(--danger);
  }
  .toast.info .toast-icon {
    color: var(--text-muted);
  }

  .toast-icon {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .toast-msg {
    flex: 1;
    min-width: 0;
    font-family: var(--font-sans);
    font-size: 0.78rem;
  }

  .toast-action {
    padding: 2px 10px;
    font-size: 0.74rem;
    font-family: var(--font-sans);
    font-weight: 500;
    border-radius: var(--radius-xs);
    background: var(--accent);
    color: var(--on-accent, #fff);
    flex-shrink: 0;
    cursor: pointer;
  }

  .toast-action:hover {
    filter: brightness(1.1);
  }

  .toast-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    background: transparent;
    color: var(--text-muted);
    border-radius: var(--radius-xs);
    flex-shrink: 0;
  }

  .toast-close:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  @keyframes toastIn {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
</style>
