<script lang="ts">
  import { untrack } from "svelte";
  import { ChevronRight } from "lucide-svelte";
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    icon?: any;
    children: Snippet;
    collapsible?: boolean;
    defaultOpen?: boolean;
  }

  let {
    title,
    icon: Icon,
    children,
    collapsible = false,
    defaultOpen = true,
  }: Props = $props();

  let open = $state(untrack(() => defaultOpen));

  function toggle() {
    if (collapsible) open = !open;
  }
</script>

<section class="section">
  {#if title}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="section-header" class:collapsible onclick={toggle}>
      <h3 class="section-title">
        {#if Icon}<Icon size={14} />{/if}
        {title}
      </h3>
      {#if collapsible}
        <span class="chevron" class:open>
          <ChevronRight size={14} />
        </span>
      {/if}
    </div>
  {/if}
  {#if !collapsible || open}
    <div class="section-body">
      {@render children()}
    </div>
  {/if}
</section>

<style>
  .section {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-md) var(--space-lg);
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-subtle);
  }

  .section-header.collapsible {
    cursor: pointer;
    user-select: none;
    -webkit-user-select: none;
  }

  .section-header.collapsible:hover {
    background: var(--bg-hover);
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    font-family: var(--font-sans);
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
    letter-spacing: -0.01em;
  }

  .chevron {
    display: flex;
    align-items: center;
    color: var(--text-muted);
    transition: transform 0.2s ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .section-body {
    padding: var(--space-lg);
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }
</style>
