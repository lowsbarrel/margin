<script lang="ts">
  interface Props {
    icon: any;
    onclick: (e: MouseEvent) => void;
    title?: string;
    size?: "sm" | "md";
    active?: boolean;
    disabled?: boolean;
    extraClass?: string;
  }

  let {
    icon: Icon,
    onclick,
    title,
    size = "md",
    active = false,
    disabled = false,
    extraClass = "",
  }: Props = $props();
  let dim = $derived(size === "sm" ? 26 : 32);
  let iconSize = $derived(size === "sm" ? 14 : 16);
</script>

<button
  class="icon-btn {extraClass}"
  class:active
  style="width: {dim}px; height: {dim}px"
  {onclick}
  {title}
  {disabled}
>
  <Icon size={iconSize} />
</button>

<style>
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: transparent;
    color: var(--text-muted);
    border-radius: var(--radius-xs);
    transition: all 0.15s ease;
    flex-shrink: 0;
  }

  .icon-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .icon-btn.active {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  .icon-btn:disabled {
    opacity: 0.5;
    pointer-events: none;
  }

  :global(.icon-btn.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }
</style>
