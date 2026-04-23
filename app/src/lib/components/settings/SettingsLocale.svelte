<script lang="ts">
  import { Section } from "$lib/ui";
  import { Globe } from "lucide-svelte";
  import { getLocale, setLocale, locales } from "$lib/paraglide/runtime.js";
  import * as m from "$lib/paraglide/messages.js";

  function handleLocaleChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    setLocale(target.value as any);
  }
</script>

<Section
  title={m.settings_language()}
  icon={Globe}
  collapsible
  defaultOpen={false}
>
  <select
    class="select-field"
    value={getLocale()}
    onchange={handleLocaleChange}
  >
    {#each locales as loc}
      <option value={loc}>{loc === "en" ? "English" : "Italiano"}</option>
    {/each}
  </select>
</Section>

<style>
  .select-field {
    padding: 8px 12px;
    font-size: 0.8rem;
    font-family: var(--font-sans);
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: border-color 0.15s ease;
    width: 100%;
  }

  .select-field:focus {
    border-color: var(--text-muted);
    outline: none;
  }
</style>
