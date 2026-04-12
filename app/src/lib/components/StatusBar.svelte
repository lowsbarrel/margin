<script lang="ts">
  import { editor } from "$lib/stores/editor.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { exportPdf } from "$lib/utils/pdfExport";
  import { IconButton } from "$lib/ui";
  import {
    CloudOff,
    Loader,
    Check,
    AlertCircle,
    Sun,
    Moon,
    LogOut,
    Settings,
    FileDown,
    RefreshCw,
    ArrowLeftRight,
    History,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  interface Props {
    onlogout?: () => void;
    onsettings?: () => void;
    onsync?: () => void;
    onswitchvault?: () => void;
    onhistory?: () => void;
    historyActive?: boolean;
  }

  let {
    onlogout,
    onsettings,
    onsync,
    onswitchvault,
    onhistory,
    historyActive = false,
  }: Props = $props();
  let exporting = $state(false);

  async function handleExportPdf() {
    const tiptap = editor.tiptap;
    if (!tiptap || exporting) return;

    exporting = true;
    try {
      await exportPdf(tiptap, m.statusbar_export_pdf_success());
    } catch (err) {
      console.error("PDF export failed:", err);
      toast.error(`PDF export failed: ${String(err)}`);
    } finally {
      exporting = false;
    }
  }
</script>

<footer class="statusbar">
  <div class="status-group">
    <span class="status-item"
      >Ln {editor.cursorLine}, Col {editor.cursorCol}</span
    >
    <span class="sep">·</span>
    <span class="status-item">{m.statusbar_markdown()}</span>
    <span class="sep">·</span>
    <span class="status-item">UTF-8</span>
  </div>

  <div class="status-group">
    <span
      class="sync-status"
      class:synced={editor.syncStatus === "synced"}
      class:syncing={editor.syncStatus === "syncing"}
      class:error={editor.syncStatus === "error"}
    >
      {#if editor.syncStatus === "synced"}
        <Check size={12} />
        <span>{m.statusbar_synced()}</span>
      {:else if editor.syncStatus === "syncing"}
        <Loader size={12} class="spin" />
        <span
          >{m.statusbar_syncing()}{#if editor.syncProgress && editor.syncProgress.total > 0}&nbsp;({editor
              .syncProgress.done}/{editor.syncProgress.total}){/if}</span
        >
      {:else if editor.syncStatus === "error"}
        <AlertCircle size={12} />
        <span>{m.statusbar_sync_error()}</span>
      {:else}
        <CloudOff size={12} />
        <span>{m.statusbar_local()}</span>
      {/if}
    </span>

    {#if onsync}
      <IconButton
        icon={RefreshCw}
        size="sm"
        onclick={onsync}
        title={m.statusbar_sync_now()}
        extraClass={editor.syncStatus === "syncing" ? "spin" : ""}
        disabled={editor.syncStatus === "syncing"}
      />
    {/if}

    {#if editor.tiptap}
      <IconButton
        icon={exporting ? Loader : FileDown}
        size="sm"
        onclick={handleExportPdf}
        title={m.statusbar_export_pdf()}
      />
    {/if}

    {#if onhistory && editor.tiptap}
      <IconButton
        icon={History}
        size="sm"
        onclick={onhistory}
        title={m.statusbar_history()}
        active={historyActive}
      />
    {/if}

    {#if onsettings}
      <IconButton
        icon={Settings}
        size="sm"
        onclick={onsettings}
        title={m.statusbar_settings()}
      />
    {/if}

    <IconButton
      icon={theme.current === "dark" ? Sun : Moon}
      size="sm"
      onclick={() => theme.toggle()}
      title={m.statusbar_toggle_theme()}
    />

    {#if onswitchvault}
      <button
        class="vault-switch"
        onclick={onswitchvault}
        title={m.statusbar_switch_vault()}
      >
        <ArrowLeftRight size={12} />
        <span class="vault-switch-name">{vault.profileName || "Vault"}</span>
      </button>
    {/if}

    {#if onlogout}
      <IconButton
        icon={LogOut}
        size="sm"
        onclick={onlogout}
        title={m.statusbar_lock_vault()}
      />
    {/if}
  </div>
</footer>

<style>
  .statusbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--space-lg);
    height: 32px;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border);
    font-size: 0.8rem;
    color: var(--text-muted);
    user-select: none;
    flex-shrink: 0;
  }

  .status-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .sep {
    color: var(--border);
  }

  .status-item {
    color: var(--text-muted);
  }

  .sync-status {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .sync-status.synced {
    color: var(--success);
  }
  .sync-status.syncing {
    color: var(--warning);
  }
  .sync-status.error {
    color: var(--danger);
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .vault-switch {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    background: none;
    color: var(--text-muted);
    font-size: 0.75rem;
    font-family: var(--font-sans);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    cursor: pointer;
    transition: all 0.15s ease;
    max-width: 140px;
  }

  .vault-switch:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }

  .vault-switch-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
