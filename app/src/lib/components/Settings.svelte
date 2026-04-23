<script lang="ts">
  import { vault } from "$lib/stores/vault.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { s3Configure, type S3Config } from "$lib/s3/bridge";
  import {
    saveSettings,
    loadSettings,
    type AppSettings,
  } from "$lib/settings/bridge";
  import {
    startAutoSync,
    stopAutoSync,
    type ConflictStrategy,
  } from "$lib/sync/s3sync";
  import { GlassModal } from "$lib/ui";
  import { listDirectory } from "$lib/fs/bridge";
  import * as m from "$lib/paraglide/messages.js";
  import ThemeEditor from "./ThemeEditor.svelte";
  import SettingsVault from "./settings/SettingsVault.svelte";
  import SettingsCloud from "./settings/SettingsCloud.svelte";
  import SettingsAttachments from "./settings/SettingsAttachments.svelte";
  import SettingsLocale from "./settings/SettingsLocale.svelte";
  import SettingsExportZip from "./settings/SettingsExportZip.svelte";
  import SettingsExportImport from "./settings/SettingsExportImport.svelte";
  import SettingsUpdates from "./settings/SettingsUpdates.svelte";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let endpoint = $state("");
  let bucket = $state("");
  let region = $state("us-east-1");
  let accessKey = $state("");
  let secretKey = $state("");
  let attachmentFolder = $state("");
  let autoSync = $state(false);
  let conflictStrategy = $state<ConflictStrategy>("local_wins");
  let vaultFolders = $state<string[]>([]);
  let saving = $state(false);

  $effect(() => {
    if (vault.vaultPath && vault.encryptionKey) {
      loadSettings(vault.vaultPath, vault.encryptionKey).then((settings) => {
        if (settings?.s3) {
          endpoint = settings.s3.endpoint;
          bucket = settings.s3.bucket;
          region = settings.s3.region;
          accessKey = settings.s3.access_key;
          secretKey = settings.s3.secret_key;
        }
        attachmentFolder = settings?.attachment_folder ?? "";
        autoSync = settings?.auto_sync ?? false;
        conflictStrategy =
          (settings?.conflict_strategy as ConflictStrategy) ?? "local_wins";
      });
      listDirectory(vault.vaultPath).then((entries) => {
        vaultFolders = entries
          .filter((e) => e.is_dir && !e.name.startsWith("."))
          .map((e) => e.name)
          .sort();
      });
    }
  });

  function getS3Config(): S3Config {
    return {
      endpoint: endpoint.trim(),
      bucket: bucket.trim(),
      region: region.trim(),
      access_key: accessKey.trim(),
      secret_key: secretKey.trim(),
    };
  }

  function getAppSettings(): AppSettings {
    const config = getS3Config();
    const hasS3 =
      config.endpoint &&
      config.bucket &&
      config.access_key &&
      config.secret_key;
    return {
      s3: hasS3 ? config : null,
      attachment_folder: attachmentFolder.trim() || null,
      auto_sync: autoSync || null,
      conflict_strategy: conflictStrategy,
    };
  }

  function handleImported(settings: AppSettings) {
    if (settings.s3) {
      endpoint = settings.s3.endpoint;
      bucket = settings.s3.bucket;
      region = settings.s3.region;
      accessKey = settings.s3.access_key;
      secretKey = settings.s3.secret_key;
    }
    attachmentFolder = settings.attachment_folder ?? "";
    autoSync = settings.auto_sync ?? false;
    conflictStrategy =
      (settings.conflict_strategy as ConflictStrategy) ?? "local_wins";
  }

  async function handleClose() {
    await handleSave();
    onclose();
  }

  async function handleSave() {
    if (!vault.vaultPath || !vault.encryptionKey) return;
    saving = true;
    try {
      const settings = getAppSettings();
      if (settings.s3) await s3Configure(settings.s3);
      await saveSettings(vault.vaultPath, vault.encryptionKey, settings);

      if (autoSync && settings.s3 && vault.vaultId && vault.encryptionKey) {
        startAutoSync(
          vault.vaultPath,
          vault.vaultId,
          vault.encryptionKey,
          settings.s3,
          undefined,
          { conflictStrategy },
        );
      } else {
        stopAutoSync();
      }

      toast.success(m.toast_settings_saved());
    } catch (err) {
      toast.error(m.toast_save_failed({ error: String(err) }));
    } finally {
      saving = false;
    }
  }

</script>

<GlassModal title={m.settings_title()} onclose={handleClose}>
  <SettingsVault />
  <SettingsCloud
    bind:endpoint
    bind:bucket
    bind:region
    bind:accessKey
    bind:secretKey
    bind:autoSync
    bind:conflictStrategy
  />
  <SettingsAttachments bind:attachmentFolder {vaultFolders} />
  <SettingsLocale />
  <ThemeEditor />
  <SettingsExportZip />
  <SettingsExportImport getSettings={getAppSettings} onimported={handleImported} />
  <SettingsUpdates />
</GlassModal>

