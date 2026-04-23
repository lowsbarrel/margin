<script lang="ts">
  import { vault } from "$lib/stores/vault.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { saveVaultProfile } from "$lib/session/bridge";
  import { Button, Input, Field, Section } from "$lib/ui";
  import { KeyRound, Eye, EyeOff, FolderOpen } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  let showPassphrase = $state(false);
  let editingVaultName = $state("");

  $effect(() => {
    editingVaultName = vault.profileName ?? "";
  });

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

<style>
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
