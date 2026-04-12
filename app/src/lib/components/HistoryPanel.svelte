<script lang="ts">
  import { vault } from "$lib/stores/vault.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import {
    listSnapshots,
    readSnapshot,
    deleteSnapshot,
    clearSnapshots,
    saveSnapshot,
    type Snapshot,
  } from "$lib/history/bridge";
  import { writeFileBytes, readFileBytes } from "$lib/fs/bridge";
  import { editor as editorStore } from "$lib/stores/editor.svelte";
  import { IconButton } from "$lib/ui";
  import {
    History,
    Trash2,
    RotateCcw,
    X,
    Clock,
    ChevronDown,
    ChevronRight,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  interface Props {
    filePath: string;
    onclose: () => void;
    onrestore?: (content: string) => void;
  }

  let { filePath, onclose, onrestore }: Props = $props();

  let snapshots = $state<Snapshot[]>([]);
  let loading = $state(true);
  let previewContent = $state<string | null>(null);
  let previewFilename = $state<string | null>(null);

  $effect(() => {
    if (filePath && vault.vaultPath) {
      loadSnapshots();
    }
  });

  async function loadSnapshots() {
    if (!vault.vaultPath) return;
    loading = true;
    try {
      snapshots = await listSnapshots(vault.vaultPath, filePath);
    } catch (err) {
      console.error("Failed to list snapshots:", err);
      toast.error(`${m.history_load_failed()}`);
    } finally {
      loading = false;
    }
  }

  async function handlePreview(snapshot: Snapshot) {
    if (!vault.vaultPath) return;
    if (previewFilename === snapshot.filename) {
      previewContent = null;
      previewFilename = null;
      return;
    }
    try {
      const bytes = await readSnapshot(
        vault.vaultPath,
        filePath,
        snapshot.filename,
      );
      previewContent = new TextDecoder().decode(bytes);
      previewFilename = snapshot.filename;
    } catch (err) {
      console.error("Failed to read snapshot:", err);
      toast.error(`${m.history_read_failed()}`);
    }
  }

  async function handleRestore(snapshot: Snapshot) {
    if (!vault.vaultPath) return;
    try {
      // Save a snapshot of the current content before restoring, so the user can undo
      try {
        const currentBytes = await readFileBytes(filePath);
        await saveSnapshot(vault.vaultPath, filePath, currentBytes);
      } catch (err) {
        console.warn("Pre-restore snapshot failed:", err);
      }

      const bytes = await readSnapshot(
        vault.vaultPath,
        filePath,
        snapshot.filename,
      );
      const content = new TextDecoder().decode(bytes);

      // Write restored content to the file
      const encoder = new TextEncoder();
      await writeFileBytes(filePath, encoder.encode(content));
      editorStore.setDirty(false);

      onrestore?.(content);
      await loadSnapshots(); // Refresh list to show the pre-restore snapshot
      toast.success(m.history_restored());
    } catch (err) {
      console.error("Restore failed:", err);
      toast.error(`${m.history_restore_failed()}`);
    }
  }

  async function handleDelete(snapshot: Snapshot) {
    if (!vault.vaultPath) return;
    try {
      await deleteSnapshot(vault.vaultPath, filePath, snapshot.filename);
      snapshots = snapshots.filter((s) => s.filename !== snapshot.filename);
      if (previewFilename === snapshot.filename) {
        previewContent = null;
        previewFilename = null;
      }
    } catch (err) {
      console.error("Delete failed:", err);
      toast.error(`${m.history_delete_failed()}`);
    }
  }

  async function handleClearAll() {
    if (!vault.vaultPath) return;
    try {
      const count = await clearSnapshots(vault.vaultPath, filePath);
      snapshots = [];
      previewContent = null;
      previewFilename = null;
      toast.success(`${m.history_cleared({ count: String(count) })}`);
    } catch (err) {
      console.error("Clear failed:", err);
    }
  }

  function formatDate(timestamp: number): string {
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    // Today: show time only
    if (diff < 86400000 && date.getDate() === now.getDate()) {
      return date.toLocaleTimeString(undefined, {
        hour: "2-digit",
        minute: "2-digit",
      });
    }
    // Yesterday
    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    if (
      date.getDate() === yesterday.getDate() &&
      date.getMonth() === yesterday.getMonth()
    ) {
      return `${m.history_yesterday()} ${date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}`;
    }
    // This year
    if (date.getFullYear() === now.getFullYear()) {
      return date.toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    }
    // Older
    return date.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  }

  // Group snapshots by day
  function groupByDay(
    items: Snapshot[],
  ): { label: string; snapshots: Snapshot[] }[] {
    const groups = new Map<string, Snapshot[]>();

    for (const snap of items) {
      const date = new Date(snap.timestamp * 1000);
      const key = date.toLocaleDateString(undefined, {
        year: "numeric",
        month: "long",
        day: "numeric",
      });
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(snap);
    }

    return Array.from(groups.entries()).map(([label, snapshots]) => ({
      label,
      snapshots,
    }));
  }

  let grouped = $derived(groupByDay(snapshots));
</script>

<div class="history-panel">
  <div class="panel-header">
    <div class="header-left">
      <History size={14} />
      <span class="header-title">{m.history_title()}</span>
      {#if snapshots.length > 0}
        <span class="count">{snapshots.length}</span>
      {/if}
    </div>
    <div class="header-actions">
      {#if snapshots.length > 0}
        <IconButton
          icon={Trash2}
          size="sm"
          onclick={handleClearAll}
          title={m.history_clear_all()}
        />
      {/if}
      <IconButton
        icon={X}
        size="sm"
        onclick={onclose}
        title={m.history_close()}
      />
    </div>
  </div>

  <div class="panel-body">
    {#if loading}
      <div class="empty">{m.history_loading()}</div>
    {:else if snapshots.length === 0}
      <div class="empty">{m.history_empty()}</div>
    {:else}
      {#each grouped as group}
        <div class="day-group">
          <div class="day-label">{group.label}</div>
          {#each group.snapshots as snapshot}
            <button
              class="snapshot-item"
              class:active={previewFilename === snapshot.filename}
              onclick={() => handlePreview(snapshot)}
            >
              <div class="snapshot-main">
                <Clock size={12} />
                <span class="snapshot-time"
                  >{formatDate(snapshot.timestamp)}</span
                >
                <span class="snapshot-size">{formatSize(snapshot.size)}</span>
              </div>
              <div
                class="snapshot-actions"
                role="presentation"
                onclick={(e) => e.stopPropagation()}
                onkeydown={(e) => e.stopPropagation()}
              >
                <IconButton
                  icon={RotateCcw}
                  size="sm"
                  onclick={() => handleRestore(snapshot)}
                  title={m.history_restore()}
                />
                <IconButton
                  icon={Trash2}
                  size="sm"
                  onclick={() => handleDelete(snapshot)}
                  title={m.history_delete()}
                />
              </div>
            </button>

            {#if previewFilename === snapshot.filename && previewContent !== null}
              <div class="preview">
                <pre>{previewContent}</pre>
              </div>
            {/if}
          {/each}
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .history-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-secondary);
    border-left: 1px solid var(--border);
    width: 300px;
    min-width: 240px;
    max-width: 400px;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-sm) var(--space-md);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    color: var(--text-secondary);
  }

  .header-title {
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .count {
    font-size: 0.7rem;
    font-weight: 500;
    background: var(--bg-tertiary);
    color: var(--text-muted);
    padding: 1px 6px;
    border-radius: 999px;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .panel-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-sm);
  }

  .empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100px;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .day-group {
    margin-bottom: var(--space-md);
  }

  .day-label {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: var(--space-xs) var(--space-sm);
    margin-bottom: var(--space-xs);
  }

  .snapshot-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: var(--space-sm) var(--space-sm);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    border-radius: var(--radius-xs);
    cursor: pointer;
    transition: background 0.15s ease;
    font-size: 0.85rem;
    text-align: left;
  }

  .snapshot-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .snapshot-item.active {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .snapshot-main {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    flex: 1;
    min-width: 0;
  }

  .snapshot-time {
    font-size: 0.8rem;
    white-space: nowrap;
  }

  .snapshot-size {
    font-size: 0.78rem;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .snapshot-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .snapshot-item:hover .snapshot-actions {
    opacity: 1;
  }

  .preview {
    margin: var(--space-xs) var(--space-sm);
    padding: var(--space-sm);
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    max-height: 300px;
    overflow: auto;
  }

  .preview pre {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    line-height: 1.5;
    color: var(--text-secondary);
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
