<script lang="ts">
  import { vault } from "$lib/stores/vault.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { s3Configure, s3TestConnection, type S3Config } from "$lib/s3/bridge";
  import {
    saveSettings,
    loadSettings,
    exportSettingsString,
    importSettingsString,
    type AppSettings,
  } from "$lib/settings/bridge";
  import { saveVaultProfile, type VaultProfile } from "$lib/session/bridge";
  import {
    syncToS3,
    startAutoSync,
    stopAutoSync,
    type ConflictStrategy,
  } from "$lib/sync/s3sync";
  import { files } from "$lib/stores/files.svelte";
  import { GlassModal, Button, Input, TextArea, Field, Section } from "$lib/ui";
  import {
    Cloud,
    Copy,
    Check,
    Upload,
    Download,
    TestTube,
    Globe,
    Paperclip,
    RefreshCw,
    KeyRound,
    FolderOpen,
    Eye,
    EyeOff,
    Archive,
    ArrowDownToLine,
  } from "lucide-svelte";
  import { listDirectory, exportVaultZip } from "$lib/fs/bridge";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import * as m from "$lib/paraglide/messages.js";
  import { getLocale, setLocale, locales } from "$lib/paraglide/runtime.js";
  import ThemeEditor from "./ThemeEditor.svelte";
  import { checkForAppUpdate } from "$lib/utils/updater";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let endpoint = $state("");
  let bucket = $state("");
  let region = $state("us-east-1");
  let accessKey = $state("");
  let secretKey = $state("");

  let testing = $state(false);
  let testOk = $state<boolean | null>(null);
  let testResult = $state("");
  let saving = $state(false);
  let syncing = $state(false);
  let exportString = $state("");
  let importString = $state("");
  let copied = $state(false);
  let attachmentFolder = $state("");
  let autoSync = $state(false);
  let conflictStrategy = $state<ConflictStrategy>("local_wins");
  let vaultFolders = $state<string[]>([]);
  let showPassphrase = $state(false);
  let editingVaultName = $state("");
  let exportingZip = $state(false);
  let checkingUpdate = $state(false);
  let updateResult = $state<string | null>(null);

  $effect(() => {
    editingVaultName = vault.profileName ?? "";
  });

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
      // Load top-level folders for the picker
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

  async function handleTest() {
    testing = true;
    testOk = null;
    testResult = "";
    try {
      await s3Configure(getS3Config());
      testResult = await s3TestConnection();
      testOk = true;
    } catch (err) {
      testResult = String(err);
      testOk = false;
    } finally {
      testing = false;
    }
  }

  async function handleSave() {
    if (!vault.vaultPath || !vault.encryptionKey) return;
    saving = true;
    try {
      const settings = getAppSettings();
      if (settings.s3) await s3Configure(settings.s3);
      await saveSettings(vault.vaultPath, vault.encryptionKey, settings);

      // Start or stop auto-sync based on setting
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

  async function handleSync() {
    if (!vault.vaultPath || !vault.vaultId || !vault.encryptionKey) return;
    const config = getS3Config();
    if (!config.endpoint || !config.bucket) {
      toast.error(m.toast_configure_s3());
      return;
    }
    syncing = true;
    try {
      await syncToS3(
        vault.vaultPath,
        vault.vaultId,
        vault.encryptionKey,
        config,
        { conflictStrategy },
      );
      if (vault.vaultPath) await files.refresh(vault.vaultPath);
      toast.success(m.toast_sync_complete());
    } catch (err) {
      toast.error(m.toast_sync_failed({ error: String(err) }));
    } finally {
      syncing = false;
    }
  }

  async function handleExport() {
    if (!vault.encryptionKey) return;
    try {
      exportString = await exportSettingsString(
        vault.encryptionKey,
        getAppSettings(),
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
      if (settings.s3) {
        endpoint = settings.s3.endpoint;
        bucket = settings.s3.bucket;
        region = settings.s3.region;
        accessKey = settings.s3.access_key;
        secretKey = settings.s3.secret_key;
        await s3Configure(settings.s3);
      }
      attachmentFolder = settings.attachment_folder ?? "";
      autoSync = settings.auto_sync ?? false;
      conflictStrategy =
        (settings.conflict_strategy as ConflictStrategy) ?? "local_wins";
      await saveSettings(vault.vaultPath, vault.encryptionKey, settings);

      // Restart auto-sync if enabled in the imported settings
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

      importString = "";
      toast.success(m.toast_import_success());
    } catch (err) {
      toast.error(m.toast_import_failed({ error: String(err) }));
    }
  }

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

  function handleLocaleChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    setLocale(target.value as any);
  }

  async function handleSaveVaultName() {
    if (!vault.vaultPath || !vault.mnemonic) return;
    const name = editingVaultName.trim() || "Vault";
    try {
      await saveVaultProfile({
        name,
        mnemonic: vault.mnemonic,
        vault_path: vault.vaultPath,
      });
      vault.profileName = name;
      toast.success(m.toast_settings_saved());
    } catch (err) {
      toast.error(String(err));
    }
  }
</script>

<GlassModal title={m.settings_title()} {onclose}>
  <Section
    title={m.settings_vault_title()}
    icon={KeyRound}
    collapsible
    defaultOpen={true}
  >
    <Field label={m.settings_vault_name()} forId="vaultName">
      <div class="vault-name-row">
        <Input
          id="vaultName"
          bind:value={editingVaultName}
          placeholder={m.login_vault_name_placeholder()}
        />
        <Button variant="secondary" onclick={handleSaveVaultName}
          >{m.settings_save_short()}</Button
        >
      </div>
    </Field>

    <Field label={m.settings_vault_passphrase()} forId="vaultPassphrase">
      <div class="passphrase-row">
        <div class="passphrase-display">
          {#if showPassphrase}
            <span class="passphrase-text">{vault.mnemonic ?? ""}</span>
          {:else}
            <span class="passphrase-hidden">••••••••••••••••••••••••</span>
          {/if}
        </div>
        <button
          class="toggle-visibility"
          onclick={() => (showPassphrase = !showPassphrase)}
          title={showPassphrase
            ? m.settings_vault_hide()
            : m.settings_vault_show()}
        >
          {#if showPassphrase}
            <EyeOff size={14} />
          {:else}
            <Eye size={14} />
          {/if}
        </button>
      </div>
    </Field>

    <Field label={m.settings_vault_location()} forId="vaultLocation">
      <div class="vault-path-display">
        <FolderOpen size={14} />
        <span class="vault-path-text">{vault.vaultPath ?? ""}</span>
      </div>
    </Field>
  </Section>

  <Section
    title={m.settings_s3_title()}
    icon={Cloud}
    collapsible
    defaultOpen={false}
  >
    <Field label={m.settings_endpoint()} forId="endpoint">
      <Input
        id="endpoint"
        bind:value={endpoint}
        placeholder={m.settings_endpoint_placeholder()}
        mono
      />
    </Field>
    <Field label={m.settings_bucket()} forId="bucket">
      <Input
        id="bucket"
        bind:value={bucket}
        placeholder={m.settings_bucket_placeholder()}
        mono
      />
    </Field>
    <Field label={m.settings_region()} forId="region">
      <Input
        id="region"
        bind:value={region}
        placeholder={m.settings_region_placeholder()}
        mono
      />
    </Field>
    <Field label={m.settings_access_key()} forId="accessKey">
      <Input
        id="accessKey"
        bind:value={accessKey}
        placeholder={m.settings_access_key_placeholder()}
        mono
      />
    </Field>
    <Field label={m.settings_secret_key()} forId="secretKey">
      <Input
        id="secretKey"
        bind:value={secretKey}
        type="password"
        placeholder="••••••••"
        mono
      />
    </Field>

    <div class="actions">
      <Button
        variant="secondary"
        icon={TestTube}
        onclick={handleTest}
        loading={testing}
      >
        {testing ? m.settings_testing() : m.settings_test()}
      </Button>
      <Button variant="primary" onclick={handleSave} loading={saving}>
        {saving ? m.settings_saving() : m.settings_save()}
      </Button>
      <Button
        variant="success"
        icon={Upload}
        onclick={handleSync}
        loading={syncing}
      >
        {syncing ? m.settings_syncing() : m.settings_sync_now()}
      </Button>
    </div>

    {#if testResult}
      <p class="test-result" class:ok={testOk}>{testResult}</p>
    {/if}

    <div class="auto-sync-row">
      <label class="toggle-label" for="autoSync">
        <RefreshCw size={14} />
        Auto-sync
      </label>
      <label class="toggle-switch">
        <input type="checkbox" id="autoSync" bind:checked={autoSync} />
        <span class="toggle-track"></span>
      </label>
    </div>
    {#if autoSync}
      <p class="hint">
        Vault will sync automatically every 5 minutes and after each save.
      </p>
    {/if}

    <Field label="Conflict resolution" forId="conflictStrategy">
      <select
        class="select-field"
        id="conflictStrategy"
        bind:value={conflictStrategy}
      >
        <option value="local_wins">Local wins</option>
        <option value="keep_newer">Keep newer</option>
      </select>
    </Field>
    {#if conflictStrategy === "keep_newer"}
      <p class="hint">
        On conflict, the file with the most recent modification time wins. The
        other copy is saved as a .sync-conflict file.
      </p>
    {:else}
      <p class="hint">
        On conflict, the local version always wins. The remote version is saved
        as a .sync-conflict file.
      </p>
    {/if}
  </Section>

  <Section
    title={m.settings_attachments_title()}
    icon={Paperclip}
    collapsible
    defaultOpen={false}
  >
    <p class="hint">{m.settings_attachments_hint()}</p>
    <Field label={m.settings_attachments_label()} forId="attachmentFolder">
      {#if vaultFolders.length > 0}
        <select
          class="select-field"
          id="attachmentFolder"
          bind:value={attachmentFolder}
        >
          <option value="">{m.settings_attachments_none()}</option>
          {#each vaultFolders as folder}
            <option value={folder}>{folder}</option>
          {/each}
        </select>
      {:else}
        <Input
          id="attachmentFolder"
          bind:value={attachmentFolder}
          placeholder="e.g. attachments"
        />
      {/if}
    </Field>
  </Section>

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

  <ThemeEditor />

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
</GlassModal>

<style>
  .actions {
    display: flex;
    gap: var(--space-sm);
    flex-wrap: wrap;
  }

  .test-result {
    font-size: 0.75rem;
    color: var(--danger);
    margin: 0;
    font-family: var(--font-sans);
    font-style: italic;
  }

  .test-result.ok {
    color: var(--success);
  }

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

  .auto-sync-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 0;
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.8rem;
    color: var(--text-primary);
    font-family: var(--font-sans);
    cursor: pointer;
  }

  .toggle-switch {
    position: relative;
    display: inline-flex;
    width: 36px;
    height: 20px;
    flex-shrink: 0;
  }

  .toggle-switch input {
    opacity: 0;
    width: 0;
    height: 0;
    position: absolute;
  }

  .toggle-track {
    position: absolute;
    inset: 0;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 100px;
    cursor: pointer;
    transition:
      background 0.2s ease,
      border-color 0.2s ease;
  }

  .toggle-track::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    background: var(--text-muted);
    border-radius: 50%;
    transition:
      transform 0.2s ease,
      background 0.2s ease;
  }

  .toggle-switch input:checked + .toggle-track {
    background: var(--text-primary);
    border-color: var(--text-primary);
  }

  .toggle-switch input:checked + .toggle-track::after {
    transform: translateX(16px);
    background: var(--bg-primary);
  }

  .vault-name-row {
    display: flex;
    gap: var(--space-sm);
    align-items: center;
  }

  .passphrase-row {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 8px 12px;
  }

  .passphrase-display {
    flex: 1;
    overflow: hidden;
  }

  .passphrase-text {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--text-primary);
    word-spacing: 0.3em;
    line-height: 1.6;
  }

  .passphrase-hidden {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--text-muted);
    letter-spacing: 0.1em;
  }

  .toggle-visibility {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    background: none;
    color: var(--text-muted);
    border: none;
    border-radius: var(--radius-xs);
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .toggle-visibility:hover {
    color: var(--text-primary);
  }

  .vault-path-display {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 8px 12px;
    color: var(--text-muted);
  }

  .vault-path-text {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
