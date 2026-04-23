<script lang="ts">
  import type { Pane } from "$lib/stores/panes.svelte";
  import { panes, fileTitle, toBreadcrumbs } from "$lib/stores/panes.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { drag } from "$lib/stores/drag.svelte";
  import Editor from "$lib/components/Editor.svelte";
  import ImageViewer from "$lib/components/ImageViewer.svelte";
  import PdfViewer from "$lib/components/PdfViewer.svelte";
  import CanvasEditor from "$lib/components/CanvasEditor.svelte";
  import GraphView from "$lib/components/GraphView.svelte";
  import { X, ChevronRight } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";
  import { handleTabMouseDown } from "$lib/utils/tab-drag";

  let {
    pane,
    paneIndex,
    onfileselect,
    onrename,
    onwikilink,
    ontabcontextmenu,
    attachmentFolder,
    dropTarget,
    ondropenter,
    ondropleave,
  }: {
    pane: Pane;
    paneIndex: number;
    onfileselect: (path: string, searchText?: string) => void;
    onrename: (from: string, to: string, isDir?: boolean) => void;
    onwikilink: (title: string) => void;
    ontabcontextmenu: (e: MouseEvent, paneIndex: number, tabIndex: number) => void;
    attachmentFolder: string | null;
    dropTarget: { paneIndex: number; zone: "left" | "center" | "right" } | null;
    ondropenter: (paneIndex: number, zone: "left" | "center" | "right") => void;
    ondropleave: (paneIndex: number, zone: "left" | "center" | "right") => void;
  } = $props();

  let paneActiveTab = $derived(
    pane.activeTabIndex >= 0 && pane.activeTabIndex < pane.tabs.length
      ? pane.tabs[pane.activeTabIndex]
      : null,
  );

  let paneCrumbs = $derived(
    paneActiveTab ? toBreadcrumbs(paneActiveTab.path, vault.vaultPath) : [],
  );
</script>

<!-- Tab Bar -->
<div class="tab-bar">
  <div class="tabs-scroll">
    {#each pane.tabs as tab, i (tab.id)}
      <div
        class="tab"
        class:active={i === pane.activeTabIndex}
        role="tab"
        tabindex={0}
        aria-selected={i === pane.activeTabIndex}
        onmousedown={(e) => {
          e.stopPropagation();
          handleTabMouseDown(e, paneIndex, i);
        }}
        oncontextmenu={(e) => ontabcontextmenu(e, paneIndex, i)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ")
            panes.switchTab(paneIndex, i);
        }}
      >
        <span class="tab-label">{fileTitle(tab.path)}</span>
        <button
          class="tab-close"
          onclick={(e) => {
            e.stopPropagation();
            panes.closeTab(paneIndex, i);
          }}
          tabindex={-1}
          aria-label={m.tab_close_label()}
        >
          <X size={12} />
        </button>
      </div>
    {/each}
  </div>
  {#if panes.list.length > 1}
    <div class="pane-controls">
      <button
        class="pane-btn"
        onclick={(e) => {
          e.stopPropagation();
          panes.closePane(paneIndex);
        }}
        title={m.pane_close()}
      >
        <X size={14} />
      </button>
    </div>
  {/if}
</div>

<!-- Breadcrumbs -->
{#if paneActiveTab && paneCrumbs.length > 0}
  <div class="breadcrumbs">
    {#each paneCrumbs as crumb, i}
      {#if i > 0}
        <ChevronRight size={12} />
      {/if}
      <span
        class="crumb"
        class:current={i === paneCrumbs.length - 1}>{crumb}</span
      >
    {/each}
  </div>
{/if}

<!-- Editor / Viewer -->
<main class="editor-area">
  {#if paneActiveTab}
    {#each pane.tabs.filter(t => t.type === "markdown") as tab (tab.id)}
      {@const isActive = tab.id === paneActiveTab?.id}
      <div class="cached-editor-slot" class:cached-hidden={!isActive}>
        <Editor
          filePath={tab.path}
          initialContent={tab.content}
          externalContentVersion={pane.externalContentVersion}
          title={fileTitle(tab.path)}
          active={isActive && paneIndex === panes.activePaneIndex}
          onrename={onrename}
          onwikilink={onwikilink}
          onsave={(content) => {
            tab.content = content;
            panes.broadcastContent(paneIndex, tab.path, content);
          }}
          {attachmentFolder}
        />
      </div>
    {/each}
    {#if paneActiveTab.type === "image" && paneActiveTab.blobUrl}
      {#key paneActiveTab.id}
        <ImageViewer
          src={paneActiveTab.blobUrl}
          alt={fileTitle(paneActiveTab.path)}
        />
      {/key}
    {:else if paneActiveTab.type === "pdf" && paneActiveTab.pdfData}
      {#key paneActiveTab.id}
        <PdfViewer data={paneActiveTab.pdfData} />
      {/key}
    {:else if paneActiveTab.type === "canvas"}
      {#key paneActiveTab.id}
        <CanvasEditor
          filePath={paneActiveTab.path}
          initialData={paneActiveTab.content}
          onsave={(content) => {
            paneActiveTab.content = content;
            panes.broadcastContent(paneIndex, paneActiveTab.path, content);
          }}
        />
      {/key}
    {:else if paneActiveTab.type === "graph"}
      {#key paneActiveTab.id}
        <GraphView onfileselect={onfileselect} />
      {/key}
    {/if}
  {:else}
    <div class="empty-state">
      <p class="empty-text">{m.editor_empty_state()}</p>
    </div>
  {/if}
</main>

<!-- Drop overlay -->
{#if drag.active && !(drag.item?.kind === "tab" && drag.item.paneIndex === paneIndex)}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="drop-overlay"
    class:file-drag={drag.item?.kind === "file"}
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="drop-zone drop-left"
      class:active={dropTarget?.paneIndex === paneIndex && dropTarget.zone === "left"}
      onmouseenter={() => ondropenter(paneIndex, "left")}
      onmouseleave={() => ondropleave(paneIndex, "left")}
    >
      <span class="drop-label">Split Left</span>
    </div>
    {#if drag.item?.kind === "tab"}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="drop-zone drop-center"
        class:active={dropTarget?.paneIndex === paneIndex && dropTarget.zone === "center"}
        onmouseenter={() => ondropenter(paneIndex, "center")}
        onmouseleave={() => ondropleave(paneIndex, "center")}
      >
        <span class="drop-label">Move Here</span>
      </div>
    {/if}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="drop-zone drop-right"
      class:active={dropTarget?.paneIndex === paneIndex && dropTarget.zone === "right"}
      onmouseenter={() => ondropenter(paneIndex, "right")}
      onmouseleave={() => ondropleave(paneIndex, "right")}
    >
      <span class="drop-label">Split Right</span>
    </div>
  </div>
{/if}

<style>
  /* ── Tab Bar ─────────────────────────────────────────── */
  .tab-bar {
    display: flex;
    align-items: center;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    height: 38px;
    min-height: 38px;
    overflow: hidden;
  }

  .tabs-scroll {
    display: flex;
    flex: 1;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }

  .tabs-scroll::-webkit-scrollbar {
    display: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    height: 38px;
    background: none;
    border-right: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 0.8rem;
    font-family: var(--font-sans);
    cursor: pointer;
    white-space: nowrap;
    transition:
      color 0.1s,
      background 0.1s;
    min-width: 36px;
    flex-shrink: 1;
    user-select: none;
  }

  .tab:hover {
    color: var(--text-secondary);
    background: var(--bg-hover);
  }

  .tab.active {
    color: var(--text-primary);
    background: var(--bg-primary);
    border-bottom: 2px solid var(--accent-link);
  }

  .tab-label {
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;
    flex-shrink: 1;
    min-width: 0;
  }

  .tab-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    background: none;
    border: none;
    border-radius: 4px;
    color: var(--text-muted);
    transition:
      color 0.1s,
      background 0.1s;
    flex-shrink: 0;
    cursor: pointer;
  }

  .tab-close:hover {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  .pane-controls {
    display: flex;
    align-items: center;
    padding: 0 6px;
    gap: 2px;
    flex-shrink: 0;
    border-left: 1px solid var(--border);
    height: 100%;
  }

  .pane-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: none;
    border: none;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    transition:
      color 0.1s,
      background 0.1s;
  }

  .pane-btn:hover {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  /* ── Breadcrumbs ─────────────────────────────────────── */
  .breadcrumbs {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 16px;
    background: var(--bg-primary);
    border-bottom: 1px solid var(--border-subtle);
    color: var(--text-muted);
    font-size: 0.78rem;
    min-height: 28px;
    overflow-x: auto;
    white-space: nowrap;
  }

  .crumb {
    color: var(--text-muted);
  }

  .crumb.current {
    color: var(--text-secondary);
  }

  /* ── Editor Area ─────────────────────────────────────── */
  .editor-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-primary);
    position: relative;
  }

  .cached-editor-slot {
    display: contents;
  }

  .cached-editor-slot.cached-hidden {
    display: none;
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 0.95rem;
  }

  /* ── Drop Overlay ────────────────────────────────────── */
  .drop-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    z-index: 100;
    pointer-events: none;
    background: color-mix(in srgb, var(--accent-link) 6%, transparent);
  }

  .drop-zone {
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.1s;
    pointer-events: auto;
  }

  .drop-left {
    flex: 30;
    border-right: 1px dashed
      color-mix(in srgb, var(--accent-link) 40%, transparent);
  }
  .drop-center {
    flex: 40;
  }
  .drop-right {
    flex: 30;
    border-left: 1px dashed
      color-mix(in srgb, var(--accent-link) 40%, transparent);
  }

  .drop-overlay.file-drag {
    background: transparent;
    justify-content: space-between;
  }
  .drop-overlay.file-drag .drop-left {
    flex: 0 0 56px;
    border-right: 2px solid
      color-mix(in srgb, var(--accent-link) 30%, transparent);
  }
  .drop-overlay.file-drag .drop-right {
    flex: 0 0 56px;
    border-left: 2px solid
      color-mix(in srgb, var(--accent-link) 30%, transparent);
  }
  .drop-overlay.file-drag .drop-left.active,
  .drop-overlay.file-drag .drop-right.active {
    background: color-mix(in srgb, var(--accent-link) 18%, transparent);
    border-color: var(--accent-link);
  }

  .drop-zone.active {
    background: color-mix(in srgb, var(--accent-link) 22%, transparent);
  }

  .drop-label {
    font-size: 0.7rem;
    font-weight: 500;
    color: var(--accent-link);
    background: var(--bg-secondary);
    padding: 3px 8px;
    border-radius: 4px;
    border: 1px solid color-mix(in srgb, var(--accent-link) 50%, transparent);
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.1s;
  }

  .drop-zone.active .drop-label {
    opacity: 1;
  }
</style>
