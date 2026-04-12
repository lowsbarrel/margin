<script lang="ts">
  import type { Snippet } from "svelte";

  type Variant = "primary" | "secondary" | "ghost" | "danger" | "success";
  type Size = "sm" | "md" | "lg";

  interface Props {
    variant?: Variant;
    size?: Size;
    disabled?: boolean;
    loading?: boolean;
    icon?: any;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
    type?: "button" | "submit";
    title?: string;
    fullWidth?: boolean;
  }

  let {
    variant = "secondary",
    size = "md",
    disabled = false,
    loading = false,
    icon: Icon,
    onclick,
    children,
    type = "button",
    title,
    fullWidth = false,
  }: Props = $props();

  let iconSize = $derived(size === "sm" ? 12 : size === "lg" ? 16 : 14);
</script>

<button
  class="btn {variant} {size}"
  class:full-width={fullWidth}
  {disabled}
  {type}
  {title}
  onclick={loading ? undefined : onclick}
>
  {#if loading}
    <span class="spinner"></span>
  {:else if Icon}
    <Icon size={iconSize} />
  {/if}
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-sm);
    font-family: var(--font-sans);
    font-weight: 500;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
    user-select: none;
    -webkit-user-select: none;
    flex-shrink: 0;
  }

  .full-width {
    width: 100%;
  }

  /* Sizes */
  .sm {
    padding: 6px 10px;
    font-size: 0.75rem;
    border-radius: var(--radius-xs);
  }
  .md {
    padding: 8px 14px;
    font-size: 0.8rem;
  }
  .lg {
    padding: 10px 18px;
    font-size: 0.85rem;
  }

  /* Variants */
  .primary {
    background: var(--text-primary);
    color: var(--bg-primary);
    border-color: transparent;
  }
  .primary:hover:not(:disabled) {
    opacity: 0.88;
  }

  .secondary {
    background: var(--glass-bg);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    color: var(--text-primary);
    border-color: var(--glass-border);
  }
  .secondary:hover:not(:disabled) {
    background: var(--bg-hover);
    border-color: var(--border);
  }

  .ghost {
    background: transparent;
    color: var(--text-secondary);
  }
  .ghost:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .danger {
    background: var(--danger);
    color: white;
  }
  .danger:hover:not(:disabled) {
    background: var(--danger-hover);
  }

  .success {
    background: var(--success);
    color: white;
  }
  .success:hover:not(:disabled) {
    opacity: 0.88;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
