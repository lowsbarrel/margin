<script lang="ts">
  import { toast } from "$lib/stores/toast.svelte";
  import { Button, Section } from "$lib/ui";
  import { ArrowDownToLine, RefreshCw } from "lucide-svelte";
  import { checkForAppUpdate } from "$lib/utils/updater";
  import * as m from "$lib/paraglide/messages.js";

  let checkingUpdate = $state(false);
  let updateResult = $state<string | null>(null);

  async function handleCheckUpdate() {
    if (checkingUpdate) return;
    checkingUpdate = true;
    updateResult = null;
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update) {
        updateResult = m.settings_updates_found({ version: update.version });
        checkForAppUpdate();
      } else {
        updateResult = m.settings_updates_none();
      }
    } catch {
      updateResult = m.settings_updates_none();
    } finally {
      checkingUpdate = false;
    }
  }
</script>

<Section
  title={m.settings_updates_title()}
  icon={ArrowDownToLine}
  collapsible
  defaultOpen={false}
>
  <Button
    variant="secondary"
    icon={RefreshCw}
    onclick={handleCheckUpdate}
    loading={checkingUpdate}
    fullWidth
  >
    {checkingUpdate
      ? m.settings_updates_checking()
      : m.settings_updates_check()}
  </Button>
  {#if updateResult}
    <p class="hint" style="margin-top: var(--space-sm);">{updateResult}</p>
  {/if}
</Section>

<style>
  .hint {
    font-size: 0.75rem;
    color: var(--text-muted);
    margin: 0;
    font-family: var(--font-sans);
    font-style: italic;
  }
</style>
