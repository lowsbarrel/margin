<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label?: string;
    hint?: string;
    forId?: string;
    children: Snippet;
  }

  let { label, hint, forId, children }: Props = $props();
</script>

<div class="field">
  {#if label}
    <label class="field-label" for={forId}>{label}</label>
  {/if}
  {@render children()}
  {#if hint}
    <p class="field-hint">{hint}</p>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
  }

  .field-label {
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--text-muted);
  }

  .field-hint {
    font-size: 0.7rem;
    color: var(--text-muted);
    margin: 0;
    font-family: var(--font-sans);
    font-style: italic;
  }
</style>
