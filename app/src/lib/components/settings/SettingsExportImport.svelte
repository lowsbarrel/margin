<script lang="ts">
  import { vault } from "$lib/stores/vault.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { s3Configure } from "$lib/s3/bridge";
  import {
    saveSettings,
    exportSettingsString,
    importSettingsString,
    type AppSettings,
  } from "$lib/settings/bridge";
  import {
    startAutoSync,
    stopAutoSync,
    type ConflictStrategy,
  } from "$lib/sync/s3sync";
  import { Button, TextArea, Section } from "$lib/ui";
  import { Upload, Download, Copy, Check } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  interface Props {
    getSettings: () => AppSettings;
    onimported: (settings: AppSettings) => void;
  }

  let { getSettings, onimported }: Props = $props();

  let exportString = $state("");
  let importString = $state("");
  let copied = $state(false);

  async function handleExport() {
    if (!vault.encryptionKey) return;
    try {
      exportString = await exportSettingsString(
        vault.encryptionKey,
        getSettings(),
      );
    } catch (err) {
      toast.error(m.toast_export_failed({ error: String(err) }));
    }
  }

  async function handleCopy() {
    await navigator.clipboard.writeText(exportString);
    copied = true;
    toast.success(m.toast_copied());
    setTimeout(() => (copied = false), 2000);
  }

  async function handleImport() {
    if (!vault.encryptionKey || !vault.vaultPath || !importString.trim())
      return;
    try {
      const settings = await importSettingsString(
        vault.encryptionKey,
        importString.trim(),
      );
      if (settings.s3) await s3Configure(settings.s3);
      await saveSettings(vault.vaultPath, vault.encryptionKey, settings);

      if (settings.auto_sync && settings.s3 && vault.vaultId && vault.encryptionKey) {
        startAutoSync(
          vault.vaultPath,
          vault.vaultId,
          vault.encryptionKey,
          settings.s3,
          undefined,
          {
            conflictStrategy:
              (settings.conflict_strategy as ConflictStrategy) ?? "local_wins",
          },
        );
      } else {
        stopAutoSync();
      }

      onimported(settings);
      importString = "";
      toast.success(m.toast_import_success());
    } catch (err) {
      toast.error(m.toast_import_failed({ error: String(err) }));
    }
  }
</script>

<Section
  title={m.settings_export_title()}
  icon={Upload}
  collapsible
  defaultOpen={false}
>
  <p class="hint">{m.settings_export_hint()}</p>
  <Button variant="secondary" onclick={handleExport} fullWidth>
    {m.settings_export_generate()}
  </Button>
  {#if exportString}
    <div class="export-box">
      <TextArea value={exportString} readonly rows={3} />
      <button class="copy-float" onclick={handleCopy}>
        {#if copied}<Check size={14} />{:else}<Copy size={14} />{/if}
      </button>
    </div>
  {/if}
</Section>

<Section
  title={m.settings_import_title()}
  icon={Download}
  collapsible
  defaultOpen={false}
>
  <p class="hint">{m.settings_import_hint()}</p>
  <TextArea
    bind:value={importString}
    placeholder={m.settings_import_placeholder()}
    rows={3}
  />
  <Button
    variant="secondary"
    icon={Download}
    onclick={handleImport}
    disabled={!importString.trim()}
    fullWidth
  >
    {m.settings_import_btn()}
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

  .export-box {
    position: relative;
  }

  .copy-float {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    background: var(--glass-bg);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    color: var(--text-muted);
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-xs);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .copy-float:hover {
    color: var(--text-primary);
  }
</style>
