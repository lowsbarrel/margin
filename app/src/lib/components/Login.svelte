<script lang="ts">
  import { onMount } from "svelte";
  import { generateMnemonic, deriveVaultKeys } from "$lib/crypto/bridge";
  import {
    setVaultDirectory,
    fileExists,
    readFileBytes,
    writeFileBytes,
    createDirectory,
  } from "$lib/fs/bridge";
  import { vault } from "$lib/stores/vault.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { deleteVaultProfile, type VaultProfile } from "$lib/session/bridge";
  import { open } from "@tauri-apps/plugin-dialog";
  import { Button, IconButton } from "$lib/ui";
  import {
    Sun,
    Moon,
    FolderOpen,
    KeyRound,
    Plus,
    Copy,
    Check,
    Trash2,
    ArrowLeft,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  let mnemonic = $state("");
  let vaultPath = $state("");
  let vaultName = $state("");
  let error = $state("");
  let generatedMnemonic = $state("");
  let showGenerated = $state(false);
  let loading = $state(false);
  let copied = $state(false);
  let autoLogging = $state(false);

  // Multi-vault state
  let profiles = $state<VaultProfile[]>([]);
  let showNewVault = $state(false);

  onMount(async () => {
    // Load all saved vault profiles
    const data = await vault.getVaultProfiles();
    profiles = data.profiles;

    // Auto-login with last used vault if available
    if (data.last_used) {
      const lastProfile = data.profiles.find(
        (p) => p.vault_path === data.last_used,
      );
      if (lastProfile) {
        autoLogging = true;
        autoLogin(lastProfile);
      }
    }

    // If no profiles, show the new vault form directly
    if (profiles.length === 0) {
      showNewVault = true;
    }
  });

  async function autoLogin(profile: VaultProfile) {
    try {
      const keys = await deriveVaultKeys(profile.mnemonic);
      await verifyOrInitVaultId(profile.vault_path, keys.vault_id);
      await setVaultDirectory(profile.vault_path);
      vault.unlock(keys, profile.vault_path, profile.mnemonic, profile.name);
    } catch (err) {
      console.warn("Auto-login failed:", err);
      vault.lock();
      autoLogging = false;
    }
  }

  async function selectProfile(profile: VaultProfile) {
    loading = true;
    error = "";
    try {
      const keys = await deriveVaultKeys(profile.mnemonic);
      await verifyOrInitVaultId(profile.vault_path, keys.vault_id);
      await setVaultDirectory(profile.vault_path);
      vault.unlock(keys, profile.vault_path, profile.mnemonic, profile.name);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function removeProfile(profile: VaultProfile) {
    try {
      await deleteVaultProfile(profile.vault_path);
      profiles = profiles.filter((p) => p.vault_path !== profile.vault_path);
      toast.success(m.login_vault_removed());
      if (profiles.length === 0) {
        showNewVault = true;
      }
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function handleGenerate() {
    try {
      generatedMnemonic = await generateMnemonic();
      showGenerated = true;
      error = "";
    } catch (e) {
      error = String(e);
    }
  }

  /**
   * On first open: write vault_id to .margin/vault.id so future logins
   * can verify that the mnemonic matches this specific vault.
   * On subsequent opens: compare and throw if mismatch.
   */
  async function verifyOrInitVaultId(
    vaultPath: string,
    vaultId: string,
  ): Promise<void> {
    const idFile = `${vaultPath}/.margin/vault.id`;
    if (await fileExists(idFile)) {
      const stored = new TextDecoder()
        .decode(await readFileBytes(idFile))
        .trim();
      if (stored !== vaultId) {
        throw new Error(m.login_error_wrong_passphrase());
      }
    } else {
      await createDirectory(`${vaultPath}/.margin`);
      await writeFileBytes(idFile, new TextEncoder().encode(vaultId));
    }
  }

  function useGenerated() {
    mnemonic = generatedMnemonic;
    showGenerated = false;
  }

  async function copyMnemonic() {
    await navigator.clipboard.writeText(generatedMnemonic);
    copied = true;
    toast.success(m.toast_copied());
    setTimeout(() => (copied = false), 2000);
  }

  async function pickDirectory() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      // Normalise to forward slashes so path handling is consistent on Windows
      vaultPath = (selected as string).replaceAll("\\", "/");
      // Auto-fill vault name from folder name if empty
      if (!vaultName.trim()) {
        const parts = vaultPath.split("/");
        vaultName = parts[parts.length - 1] || "";
      }
    }
  }

  async function handleOpen() {
    if (!mnemonic.trim()) {
      error = m.login_error_no_mnemonic();
      return;
    }
    if (!vaultPath.trim()) {
      error = m.login_error_no_path();
      return;
    }

    loading = true;
    error = "";
    try {
      const name = vaultName.trim() || vaultPath.split("/").pop() || "Vault";
      const keys = await deriveVaultKeys(mnemonic.trim());
      await verifyOrInitVaultId(vaultPath, keys.vault_id);
      await setVaultDirectory(vaultPath);
      vault.unlock(keys, vaultPath, mnemonic.trim(), name);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function goBack() {
    showNewVault = false;
    mnemonic = "";
    vaultPath = "";
    vaultName = "";
    error = "";
    showGenerated = false;
    generatedMnemonic = "";
  }
</script>

<div class="login">
  <div class="login-shell">
    {#if autoLogging}
      <div class="auto-login">
        <div class="brand">
          <img src="/logo.svg" alt="Margin logo" class="brand-logo" />
          <h1>{m.app_name()}</h1>
        </div>
        <span class="auto-login-text">{m.login_auto_opening()}</span>
      </div>
    {:else}
      <div class="top-bar">
        <IconButton
          icon={theme.current === "dark" ? Sun : Moon}
          onclick={() => theme.toggle()}
          title={m.statusbar_toggle_theme()}
        />
      </div>

      <div class="brand">
        <img src="/logo.svg" alt="Margin logo" class="brand-logo" />
        <h1>{m.app_name()}</h1>
        <p class="tagline">{m.app_tagline()}</p>
      </div>

      {#if !showNewVault && profiles.length > 0}
        <!-- Vault selector -->
        <div class="glass-card vault-list">
          <p class="section-label">{m.login_choose_vault()}</p>
          {#each profiles as profile}
            <div
              class="vault-card"
              onclick={() => selectProfile(profile)}
              role="button"
              tabindex="0"
              onkeydown={(e: KeyboardEvent) => {
                if (e.key === "Enter") selectProfile(profile);
              }}
            >
              <div class="vault-info">
                <span class="vault-name">{profile.name}</span>
                <span class="vault-path-small">{profile.vault_path}</span>
              </div>
              <button
                class="vault-remove"
                onclick={(e: MouseEvent) => {
                  e.stopPropagation();
                  removeProfile(profile);
                }}
                title={m.login_remove_vault()}
              >
                <Trash2 size={14} />
              </button>
            </div>
          {/each}

          {#if error}
            <p class="error">{error}</p>
          {/if}

          <div class="divider"></div>
          <button
            class="generate-btn"
            onclick={() => {
              showNewVault = true;
            }}
          >
            <Plus size={14} />
            {m.login_add_vault()}
          </button>
        </div>
      {:else}
        <!-- New vault form -->
        <div class="glass-card">
          {#if profiles.length > 0}
            <button class="back-btn" onclick={goBack}>
              <ArrowLeft size={14} />
              {m.login_back_to_vaults()}
            </button>
          {/if}

          <div class="field">
            <label for="vault-name">
              {m.login_vault_name_label()}
            </label>
            <input
              id="vault-name"
              type="text"
              bind:value={vaultName}
              placeholder={m.login_vault_name_placeholder()}
              class="text-input"
            />
          </div>

          <div class="field">
            <label for="mnemonic">
              <KeyRound size={14} />
              {m.login_passphrase_label()}
            </label>
            <textarea
              id="mnemonic"
              bind:value={mnemonic}
              placeholder={m.login_passphrase_placeholder()}
              rows="3"
              spellcheck="false"
            ></textarea>
          </div>

          <div class="field">
            <label for="vault-path">
              <FolderOpen size={14} />
              {m.login_storage_label()}
            </label>
            <button class="path-btn" onclick={pickDirectory}>
              {#if vaultPath}
                <span class="path-text">{vaultPath}</span>
              {:else}
                <span class="path-placeholder"
                  >{m.login_storage_placeholder()}</span
                >
              {/if}
            </button>
          </div>

          {#if error}
            <p class="error">{error}</p>
          {/if}

          <Button variant="primary" fullWidth onclick={handleOpen} {loading}>
            {loading ? m.login_opening() : m.login_open_vault()}
          </Button>

          <div class="divider"></div>

          {#if showGenerated}
            <div class="generated">
              <p class="generated-label">{m.login_generated_hint()}</p>
              <div class="generated-words">
                <p>{generatedMnemonic}</p>
                <button
                  class="copy-btn"
                  onclick={copyMnemonic}
                  aria-label="Copy"
                >
                  {#if copied}
                    <Check size={14} />
                  {:else}
                    <Copy size={14} />
                  {/if}
                </button>
              </div>
              <Button variant="primary" fullWidth onclick={useGenerated}>
                {m.login_use_passphrase()}
              </Button>
            </div>
          {:else}
            <button class="generate-btn" onclick={handleGenerate}>
              <Plus size={14} />
              {m.login_generate_new()}
            </button>
          {/if}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .login {
    height: 100vh;
    overflow-y: auto;
    padding: 2rem;
    position: relative;
  }

  .login-shell {
    width: min(100%, 380px);
    margin: 0 auto;
    min-height: calc(100vh - 4rem);
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: center;
    position: relative;
  }

  .auto-login {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.5rem;
  }

  .auto-login-text {
    color: var(--text-muted);
    font-size: 0.9rem;
    font-style: italic;
  }

  .top-bar {
    position: fixed;
    top: 1rem;
    right: 1rem;
  }

  .brand {
    text-align: center;
    margin-bottom: 2rem;
  }

  .brand-logo {
    width: 80px;
    height: 80px;
    margin-bottom: 0.75rem;
  }

  .brand h1 {
    font-family: var(--font-sans);
    font-size: 1.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--text-primary);
  }

  .tagline {
    font-family: var(--font-sans);
    font-size: 0.8rem;
    font-style: italic;
    color: var(--text-muted);
    margin-top: 0.4rem;
  }

  .glass-card {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    background: var(--glass-bg);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
    border: 1px solid var(--glass-border);
    border-radius: var(--radius);
    box-shadow: var(--glass-shadow);
    padding: var(--space-xl);
  }

  .vault-list {
    gap: 0.5rem;
  }

  .section-label {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text-muted);
    font-family: var(--font-sans);
    margin-bottom: 0.25rem;
  }

  .vault-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    text-align: left;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 10px 14px;
    cursor: pointer;
    transition:
      border-color 0.15s ease,
      background 0.15s ease;
  }

  .vault-card:hover {
    border-color: var(--text-muted);
    background: var(--bg-secondary);
  }

  .vault-card:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .vault-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
  }

  .vault-name {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-primary);
    font-family: var(--font-sans);
  }

  .vault-path-small {
    font-size: 0.7rem;
    color: var(--text-muted);
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .vault-remove {
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

  .vault-remove:hover {
    color: var(--danger);
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: none;
    color: var(--text-secondary);
    font-size: 0.8rem;
    padding: 4px 0;
    font-family: var(--font-sans);
    cursor: pointer;
  }

  .back-btn:hover {
    color: var(--text-primary);
  }

  .text-input {
    font-family: var(--font-sans);
    font-size: 0.9rem;
    line-height: 1.5;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 10px 14px;
    transition: border-color 0.15s ease;
    width: 100%;
    caret-color: var(--accent);
  }

  .text-input:focus {
    border-color: var(--accent-link);
    outline: none;
  }

  .text-input::placeholder {
    color: var(--text-muted);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .field label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--text-muted);
  }

  textarea {
    resize: none;
    font-family: var(--font-mono);
    font-size: 0.85rem;
    line-height: 1.5;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 10px 14px;
    transition: border-color 0.15s ease;
    caret-color: var(--accent);
  }

  textarea:focus {
    border-color: var(--accent-link);
    outline: none;
  }

  textarea::placeholder {
    color: var(--text-muted);
  }

  .path-btn {
    width: 100%;
    text-align: left;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    padding: 10px 14px;
    font-size: 0.85rem;
    border-radius: var(--radius-sm);
    transition: border-color 0.15s ease;
  }

  .path-btn:hover {
    border-color: var(--text-muted);
  }

  .path-text {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .path-placeholder {
    color: var(--text-muted);
  }

  .divider {
    border-top: 1px solid var(--glass-border);
    margin: 0.25rem 0;
  }

  .error {
    color: var(--danger);
    font-size: 0.8rem;
    font-family: var(--font-sans);
    font-style: italic;
  }

  .generated {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .generated-label {
    font-size: 0.8rem;
    color: var(--text-secondary);
    font-family: var(--font-sans);
    font-style: italic;
  }

  .generated-words {
    position: relative;
    background: var(--bg-tertiary);
    border: 1px dashed var(--border);
    border-radius: var(--radius-sm);
    padding: 0.75rem;
    padding-right: 2.5rem;
    font-family: var(--font-mono);
    font-size: 0.85rem;
    word-spacing: 0.3em;
    line-height: 1.6;
    color: var(--text-primary);
  }

  .copy-btn {
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
    background: none;
    color: var(--text-muted);
    padding: 4px;
    display: flex;
  }

  .copy-btn:hover {
    color: var(--text-primary);
  }

  .generate-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    background: none;
    color: var(--text-secondary);
    font-size: 0.85rem;
    padding: 8px;
    font-family: var(--font-sans);
  }

  .generate-btn:hover {
    color: var(--text-primary);
  }

  @media (max-height: 760px) {
    .login-shell {
      min-height: auto;
      margin: 0 auto;
      justify-content: flex-start;
      padding-top: 2.5rem;
      padding-bottom: 1rem;
    }
  }
</style>
