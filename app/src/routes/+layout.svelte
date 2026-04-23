<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";

  let { children }: { children: Snippet } = $props();

  onMount(() => {
    // Prevent Escape from exiting native fullscreen on macOS (capture phase).
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  });
</script>

{@render children()}
