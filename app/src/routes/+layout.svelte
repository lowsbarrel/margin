<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";

  let { children }: { children: Snippet } = $props();

  onMount(() => {
    // Prevent Escape from exiting native fullscreen on macOS.
    // This runs in the capture phase so it fires before any other handlers.
    // Existing Escape handlers still work because they check e.key, not the default action.
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
