<script lang="ts">
  import { vault } from "$lib/stores/vault.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { exportVaultZip } from "$lib/fs/bridge";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { Button, Section } from "$lib/ui";
  import { Archive } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  let exportingZip = $state(false);

  async function handleExportZip() {
    if (!vault.vaultPath || exportingZip) return;
    const vaultName = vault.profileName ?? "vault";
    const dest = await saveDialog({
      title: m.settings_export_zip_dialog_title(),
      defaultPath: `${vaultName}.zip`,
      filters: [{ name: "ZIP", extensions: ["zip"] }],
    });
    if (!dest) return;
    exportingZip = true;
    try {
      await exportVaultZip(vault.vaultPath, dest);
      toast.success(m.toast_zip_exported());
    } catch (err) {
      toast.error(m.toast_zip_export_failed({ error: String(err) }));
    } finally {
      exportingZip = false;
    }
  }
</script>

<Section
  title={m.settings_export_zip_title()}
  icon={Archive}
  collapsible
  defaultOpen={false}
>
  <p class="hint">{m.settings_export_zip_hint()}</p>
  <Button
    variant="secondary"
    icon={Archive}
    onclick={handleExportZip}
    loading={exportingZip}
    fullWidth
  >
    {exportingZip
      ? m.settings_export_zip_exporting()
      : m.settings_export_zip_btn()}
  </Button>
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
