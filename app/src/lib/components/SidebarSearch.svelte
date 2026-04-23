<script lang="ts">
  import {
    searchFiles,
    searchFileContents,
    replaceInFile,
    type FsEntry,
    type ContentMatch,
  } from "$lib/fs/bridge";
  import { files } from "$lib/stores/files.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { tags as tagsStore } from "$lib/stores/tags.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { IconButton } from "$lib/ui";
  import {
    Search,
    FileText,
    Replace,
    ReplaceAll,
    CaseSensitive,
    ChevronDown,
    ChevronRight,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";
  import {
    displayPath,
    displayName,
    highlightMatch,
  } from "$lib/utils/sidebar-ops";

  interface Props {
    onfileselect: (path: string, searchText?: string) => void;
    focusSearch: boolean;
    panelOpen: boolean;
  }

  let { onfileselect, focusSearch = $bindable(false), panelOpen }: Props = $props();

  let searchQuery = $state("");
  let searchResults = $state<FsEntry[]>([]);
  let contentResults = $state<ContentMatch[]>([]);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let searchGeneration = 0;
  let searching = $state(false);
  let searchInput = $state<HTMLInputElement | null>(null);
  let replaceQuery = $state("");
  let showReplace = $state(false);
  let caseSensitive = $state(false);
  let collapsedFiles = $state<Set<string>>(new Set());

  let allTags = $derived(tagsStore.items);
  let tagsLoading = $derived(tagsStore.loading);
  let selectedTag = $state<string | null>(null);
  let tagSearchQuery = $state("");

  let filteredTags = $derived.by(() => {
    const q = tagSearchQuery.trim().toLowerCase();
    if (!q) return allTags;
    return allTags.filter((t) => t.tag.includes(q));
  });

  let selectedTagFiles = $derived.by(() => {
    if (!selectedTag) return [];
    return allTags.find((t) => t.tag === selectedTag)?.files ?? [];
  });

  let isTagMode = $derived(searchQuery.trimStart().startsWith("#"));

  async function loadTags() {
    if (!vault.vaultPath) return;
    await tagsStore.load(vault.vaultPath);
  }

  $effect(() => {
    if (panelOpen && vault.vaultPath && isTagMode) {
      loadTags();
    }
  });

  let groupedResults = $derived.by(() => {
    const groups = new Map<string, { name: string; matches: ContentMatch[] }>();
    for (const match of contentResults) {
      if (!groups.has(match.path)) {
        groups.set(match.path, { name: match.name, matches: [] });
      }
      groups.get(match.path)!.matches.push(match);
    }
    return groups;
  });

  $effect(() => {
    if (panelOpen && searchInput) {
      searchInput.focus();
    }
  });

  $effect(() => {
    if (focusSearch) {
      focusSearch = false;
    }
  });

  function handleSearchInput() {
    const q = searchQuery.trim();
    if (!q) {
      searchResults = [];
      contentResults = [];
      selectedTag = null;
      tagSearchQuery = "";
      return;
    }
    if (q.startsWith("#")) {
      searchResults = [];
      contentResults = [];
      selectedTag = null;
      tagSearchQuery = q.slice(1);
      return;
    }
    tagSearchQuery = "";
    selectedTag = null;
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(async () => {
      if (!vault.vaultPath) return;
      searching = true;
      const gen = ++searchGeneration;
      try {
        const [fileResults, contents] = await Promise.all([
          searchFiles(vault.vaultPath, q),
          searchFileContents(vault.vaultPath, q, caseSensitive),
        ]);
        if (gen !== searchGeneration) return;
        searchResults = fileResults;
        contentResults = contents;
      } catch (err) {
        if (gen !== searchGeneration) return;
        console.warn("Search failed:", err);
        searchResults = [];
        contentResults = [];
      } finally {
        if (gen === searchGeneration) searching = false;
      }
    }, 100);
  }

  function toggleFileCollapse(path: string) {
    const next = new Set(collapsedFiles);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    collapsedFiles = next;
  }

  async function handleReplaceInFile(path: string) {
    if (!replaceQuery && replaceQuery !== "") return;
    try {
      const count = await replaceInFile(path, searchQuery, replaceQuery, caseSensitive);
      if (count > 0) {
        toast.success(m.toast_replaced({ count: String(count) }));
        handleSearchInput();
      }
    } catch (err) {
      toast.error(m.toast_replace_failed({ error: String(err) }));
    }
  }

  async function handleReplaceAll() {
    if (!vault.vaultPath) return;
    const paths = [...groupedResults.keys()];
    let total = 0;
    for (const path of paths) {
      try {
        const count = await replaceInFile(path, searchQuery, replaceQuery, caseSensitive);
        total += count;
      } catch (err) {
        console.warn(`Replace in ${path} failed:`, err);
      }
    }
    if (total > 0) {
      toast.success(m.toast_replaced_in_files({ count: String(total), files: String(paths.length) }));
      handleSearchInput();
    }
  }

  function toggleCaseSensitive() {
    caseSensitive = !caseSensitive;
    if (searchQuery.trim()) handleSearchInput();
  }
</script>

<div class="panel-header">
  <span class="panel-title">{m.sidebar_search_title()}</span>
  {#if !isTagMode}
    <div class="panel-actions">
      <IconButton
        icon={Replace}
        size="sm"
        onclick={() => (showReplace = !showReplace)}
        title={m.sidebar_toggle_replace()}
        active={showReplace}
      />
    </div>
  {/if}
</div>

<div class="search-box">
  <Search size={14} />
  <input
    class="search-input"
    bind:this={searchInput}
    bind:value={searchQuery}
    oninput={handleSearchInput}
    placeholder={m.sidebar_search_placeholder()}
  />
  {#if !isTagMode}
    <button
      class="search-toggle-btn"
      class:active={caseSensitive}
      onclick={toggleCaseSensitive}
      title={m.sidebar_case_sensitive()}
    >
      <CaseSensitive size={14} />
    </button>
  {/if}
</div>

{#if showReplace && !isTagMode}
  <div class="search-box replace-box">
    <Replace size={14} />
    <input
      class="search-input"
      bind:value={replaceQuery}
      placeholder={m.sidebar_replace_placeholder()}
    />
    <button
      class="search-toggle-btn"
      onclick={handleReplaceAll}
      title={m.sidebar_replace_all_files()}
      disabled={contentResults.length === 0}
    >
      <ReplaceAll size={14} />
    </button>
  </div>
{/if}

<div class="panel-content">
  {#if isTagMode}
    {#if selectedTag}
      <div class="tag-back-row">
        <button
          class="tag-back-btn"
          onclick={() => {
            selectedTag = null;
            searchQuery = "#";
            tagSearchQuery = "";
          }}
        >
          <ChevronRight size={12} style="transform:rotate(180deg)" />
          <span class="tag-chip tag-chip-active" style="font-size:0.75rem">#{selectedTag}</span>
        </button>
        <span class="tag-file-count"
          >{selectedTagFiles.length}
          {m.tags_files({ count: selectedTagFiles.length })}</span
        >
      </div>
      {#if selectedTagFiles.length === 0}
        <div class="search-empty">{m.tags_no_files()}</div>
      {/if}
      {#each selectedTagFiles as filePath (filePath)}
        {@const name = filePath.split("/").pop() ?? filePath}
        <button
          class="search-result"
          class:active={files.activeFile === filePath}
          onclick={() => onfileselect(filePath)}
        >
          <FileText size={14} />
          <div class="search-result-text">
            <span class="search-result-name">{displayName(name)}</span>
            {#if displayPath(filePath, vault.vaultPath)}
              <span class="search-result-path">{displayPath(filePath, vault.vaultPath)}</span>
            {/if}
          </div>
        </button>
      {/each}
    {:else if tagsLoading}
      <div class="search-empty">{m.tags_loading()}</div>
    {:else if filteredTags.length === 0}
      <div class="search-empty">{m.tags_empty()}</div>
    {:else}
      <div class="tag-cloud">
        {#each filteredTags as info (info.tag)}
          <button
            class="tag-chip"
            onclick={() => {
              selectedTag = info.tag;
              searchQuery = `#${info.tag}`;
            }}
            title="#{info.tag} — {info.count} {m.tags_files({ count: info.count })}"
          >
            #{info.tag}
            <span class="tag-count">{info.count}</span>
          </button>
        {/each}
      </div>
    {/if}
  {:else}
    {#if searchQuery.trim() && contentResults.length === 0 && searchResults.length === 0 && !searching}
      <div class="search-empty">{m.sidebar_no_results()}</div>
    {/if}

    {#if contentResults.length > 0}
      <div class="search-summary">
        {m.sidebar_results_summary({ count: String(contentResults.length), files: String(groupedResults.size) })}
      </div>
    {/if}

    {#each [...groupedResults] as [filePath, group] (filePath)}
      <div class="search-file-group">
        <button class="search-file-header" onclick={() => toggleFileCollapse(filePath)}>
          {#if collapsedFiles.has(filePath)}
            <ChevronRight size={12} />
          {:else}
            <ChevronDown size={12} />
          {/if}
          <FileText size={14} />
          <span class="search-file-name">{displayName(group.name)}</span>
          <span class="search-file-count">{group.matches.length}</span>
          {#if showReplace}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span
              class="search-file-replace"
              onclick={(e) => { e.stopPropagation(); handleReplaceInFile(filePath); }}
              title={m.sidebar_replace_all_in_file()}
            >
              <Replace size={12} />
            </span>
          {/if}
        </button>
        {#if !collapsedFiles.has(filePath)}
          {#each group.matches as match}
            <button class="search-match-item" onclick={() => onfileselect(match.path, searchQuery)}>
              <span class="search-match-line">L{match.line}</span>
              <span class="search-match-context">{@html highlightMatch(match.context, searchQuery, caseSensitive)}</span>
            </button>
          {/each}
        {/if}
      </div>
    {/each}

    {#if searchResults.length > 0 && contentResults.length > 0}
      <div class="search-section-divider">File names</div>
    {/if}

    {#each searchResults as result (result.path)}
      <button
        class="search-result"
        class:active={files.activeFile === result.path}
        onclick={() => onfileselect(result.path)}
      >
        <FileText size={14} />
        <div class="search-result-text">
          <span class="search-result-name">{displayName(result.name)}</span>
          {#if displayPath(result.path, vault.vaultPath)}
            <span class="search-result-path">{displayPath(result.path, vault.vaultPath)}</span>
          {/if}
        </div>
      </button>
    {/each}
  {/if}
</div>

<style>
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-lg) var(--space-lg) var(--space-sm);
  }

  .panel-title {
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-muted);
  }

  .panel-actions {
    display: flex;
    gap: 2px;
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    padding: 0 var(--space-sm) var(--space-sm);
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 6px 10px;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
    font-size: 0.9rem;
    flex-shrink: 0;
  }

  .search-box:focus-within {
    color: var(--text-secondary);
  }

  .search-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 0.9rem;
    font-family: var(--font-sans);
    padding: 0;
    caret-color: var(--accent);
  }

  .search-input::placeholder {
    color: var(--text-muted);
  }

  .search-empty {
    padding: 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .search-result {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    background: none;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    font-size: 0.9rem;
    cursor: pointer;
    text-align: left;
  }

  .search-result:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .search-result.active {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .search-result-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .search-result-name {
    font-size: 0.9rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .search-result-path {
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .replace-box {
    border-top: none;
    margin-top: 0;
  }

  .search-toggle-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-muted);
    cursor: pointer;
    margin-right: 2px;
    flex-shrink: 0;
    transition: color 0.12s, background 0.12s;
  }

  .search-toggle-btn:hover:not(:disabled) {
    color: var(--text-primary);
  }

  .search-toggle-btn.active {
    color: var(--accent-link);
    background: var(--bg-tertiary);
  }

  .search-toggle-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .search-summary {
    padding: 4px 10px;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .search-section-divider {
    padding: 8px 10px 4px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    border-top: 1px solid var(--border-subtle);
    margin-top: 4px;
  }

  .search-file-group {
    margin-bottom: 2px;
  }

  .search-file-header {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    padding: 4px 6px;
    background: none;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    font-size: 0.85rem;
    cursor: pointer;
    text-align: left;
  }

  .search-file-header:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .search-file-name {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
  }

  .search-file-count {
    font-size: 0.7rem;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    padding: 1px 5px;
    border-radius: 8px;
    flex-shrink: 0;
  }

  .search-file-replace {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: var(--radius-xs);
    color: var(--text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition: color 0.12s, background 0.12s;
  }

  .search-file-replace:hover {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  .search-match-item {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    width: 100%;
    padding: 3px 8px 3px 26px;
    background: none;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    text-align: left;
    line-height: 1.4;
  }

  .search-match-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .search-match-line {
    flex-shrink: 0;
    font-size: 0.7rem;
    color: var(--text-muted);
    font-family: var(--font-mono);
    min-width: 28px;
    padding-top: 1px;
  }

  .search-match-context {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .search-match-context :global(.search-highlight) {
    color: var(--text-primary);
    background: rgba(234, 179, 8, 0.3);
    border-radius: 2px;
    padding: 0 1px;
  }

  .tag-cloud {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 10px;
  }

  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 0.78rem;
    font-family: var(--font-mono);
    color: var(--text-secondary);
    cursor: pointer;
    transition: border-color 0.12s, color 0.12s, background 0.12s;
  }

  .tag-chip:hover,
  .tag-chip-active {
    border-color: var(--accent, var(--text-muted));
    color: var(--text-primary);
    background: var(--bg-secondary);
  }

  .tag-count {
    font-size: 0.7rem;
    color: var(--text-muted);
    background: var(--bg-primary);
    border-radius: 999px;
    padding: 0 5px;
    font-family: var(--font-sans);
  }

  .tag-back-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tag-back-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    color: var(--text-secondary);
    font-size: 0.8rem;
    cursor: pointer;
    padding: 2px 0;
  }

  .tag-back-btn:hover {
    color: var(--text-primary);
  }

  .tag-file-count {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-family: var(--font-sans);
  }
</style>
