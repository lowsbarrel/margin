<script lang="ts">
  import { vault } from "$lib/stores/vault.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { files } from "$lib/stores/files.svelte";
  import { s3Configure, s3TestConnection, type S3Config } from "$lib/s3/bridge";
  import {
    syncToS3,
    type ConflictStrategy,
  } from "$lib/sync/s3sync";
  import { Button, Input, Field, Section } from "$lib/ui";
  import { Cloud, TestTube, Upload, RefreshCw } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  interface Props {
    endpoint: string;
    bucket: string;
    region: string;
    accessKey: string;
    secretKey: string;
    autoSync: boolean;
    conflictStrategy: ConflictStrategy;
  }

  let {
    endpoint = $bindable(),
    bucket = $bindable(),
    region = $bindable(),
    accessKey = $bindable(),
    secretKey = $bindable(),
    autoSync = $bindable(),
    conflictStrategy = $bindable(),
  }: Props = $props();

  let testing = $state(false);
  let testOk = $state<boolean | null>(null);
  let testResult = $state("");
  let syncing = $state(false);

  function getS3Config(): S3Config {
    return {
      endpoint: endpoint.trim(),
      bucket: bucket.trim(),
      region: region.trim(),
      access_key: accessKey.trim(),
      secret_key: secretKey.trim(),
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
</script>

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
      {m.settings_auto_sync()}
    </label>
    <label class="toggle-switch">
      <input type="checkbox" id="autoSync" bind:checked={autoSync} />
      <span class="toggle-track"></span>
    </label>
  </div>
  {#if autoSync}
    <p class="hint">{m.settings_auto_sync_hint()}</p>
  {/if}

  <Field label={m.settings_conflict_resolution()} forId="conflictStrategy">
    <select
      class="select-field"
      id="conflictStrategy"
      bind:value={conflictStrategy}
    >
      <option value="local_wins">{m.settings_conflict_local_wins()}</option>
      <option value="keep_newer">{m.settings_conflict_keep_newer()}</option>
    </select>
  </Field>
  {#if conflictStrategy === "keep_newer"}
    <p class="hint">{m.settings_conflict_hint_newer()}</p>
  {:else}
    <p class="hint">{m.settings_conflict_hint_local()}</p>
  {/if}
</Section>

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
