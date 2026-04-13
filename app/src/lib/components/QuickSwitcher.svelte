<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import {
    searchFiles,
    type FsEntry,
    type TagInfo,
  } from "$lib/fs/bridge";
  import { tags as tagsStore } from "$lib/stores/tags.svelte";
  import { FileText, Image, FileType, Hash, Search } from "lucide-svelte";
  import { IMAGE_EXTS } from "$lib/utils/mime";
  import * as m from "$lib/paraglide/messages.js";

  interface Props {
    onselect: (path: string) => void;
    onclose: () => void;
  }

  let { onselect, onclose }: Props = $props();

  let query = $state("");
  let results = $state<FsEntry[]>([]);
  let selectedIndex = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);
  let searching = $state(false);
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  // Tag mode
  let allTags = $state<TagInfo[]>([]);
  let tagResults = $state<{ entry: FsEntry; tag: string }[]>([]);
  let isTagMode = $derived(query.trimStart().startsWith("#"));

  // Unified flattened result list determines keyboard navigation target
  // In file mode: results[]; in tag mode: tagResults[]
  let resultCount = $derived(isTagMode ? tagResults.length : results.length);

  function getIcon(path: string) {
    const ext = path.slice(path.lastIndexOf(".")).toLowerCase();
    if (ext === ".md") return FileText;
    if (IMAGE_EXTS.has(ext)) return Image;
    if (ext === ".pdf") return FileType;
    return Hash;
  }

  function relativePath(absPath: string): string {
    if (!vault.vaultPath) return absPath;
    const rel = absPath.slice(vault.vaultPath.length + 1);
    return rel;
  }

  function displayName(entry: FsEntry): string {
    return entry.name.replace(/\.md$/, "");
  }

  function parentPath(entry: FsEntry): string {
    const rel = relativePath(entry.path);
    const lastSlash = rel.lastIndexOf("/");
    if (lastSlash <= 0) return "";
    return rel.slice(0, lastSlash);
  }

  // Fuzzy match scoring — returns -1 if no match, higher = better
  function fuzzyScore(query: string, target: string): number {
    const q = query.toLowerCase();
    const t = target.toLowerCase();

    // Fast reject: every char in query must exist in target
    for (let i = 0; i < q.length; i++) {
      if (t.indexOf(q[i]) === -1) return -1;
    }

    // Exact substring match gets high score
    const substringIdx = t.indexOf(q);
    if (substringIdx !== -1) {
      // Bonus for matching at start
      return 1000 - substringIdx + (q.length / t.length) * 500;
    }

    // Fuzzy character matching
    let qi = 0;
    let score = 0;
    let lastMatchIdx = -1;

    for (let ti = 0; ti < t.length && qi < q.length; ti++) {
      if (t[ti] === q[qi]) {
        score += 10;
        // Consecutive match bonus
        if (lastMatchIdx === ti - 1) score += 15;
        // Start-of-word bonus
        if (
          ti === 0 ||
          t[ti - 1] === "/" ||
          t[ti - 1] === " " ||
          t[ti - 1] === "-" ||
          t[ti - 1] === "_"
        ) {
          score += 20;
        }
        lastMatchIdx = ti;
        qi++;
      }
    }

    // All query chars must match
    if (qi < q.length) return -1;

    // Penalty for longer targets
    score -= (t.length - q.length) * 0.5;

    return score;
  }

  async function doSearch(q: string) {
    if (!vault.vaultPath) return;
    const trimmed = q.trim();
    if (!trimmed) {
      results = [];
      tagResults = [];
      return;
    }

    if (trimmed.startsWith("#")) {
      await doTagSearch(trimmed.slice(1).toLowerCase());
      return;
    }

    searching = true;
    try {
      const raw = await searchFiles(vault.vaultPath, trimmed);
      // Filter out directories first, then score with fuzzy matching.
      // Limit scoring to first 200 candidates to avoid O(n) on huge vaults.
      const files = raw.filter((e) => !e.is_dir);
      const candidates = files.length > 200 ? files.slice(0, 200) : files;
      const scored = candidates
        .map((e) => ({
          entry: e,
          score: Math.max(
            fuzzyScore(trimmed, e.name),
            fuzzyScore(trimmed, relativePath(e.path)),
          ),
        }))
        .filter((s) => s.score > -1)
        .sort((a, b) => b.score - a.score)
        .slice(0, 20);

      results = scored.map((s) => s.entry);
      selectedIndex = 0;
    } finally {
      searching = false;
    }
  }

  async function doTagSearch(tagQuery: string) {
    if (!vault.vaultPath) return;
    searching = true;
    try {
      // Lazily load tags via shared store
      if (allTags.length === 0) {
        allTags = await tagsStore.load(vault.vaultPath);
      }
      // Filter matching tags (prefix match)
      const matchingTags = tagQuery
        ? allTags.filter((t) => t.tag.startsWith(tagQuery))
        : allTags;

      // Collect files from matching tags, deduplicate
      const seen = new Set<string>();
      const flat: { entry: FsEntry; tag: string }[] = [];
      for (const tagInfo of matchingTags.slice(0, 10)) {
        for (const filePath of tagInfo.files) {
          if (seen.has(filePath)) continue;
          seen.add(filePath);
          const name = filePath.split("/").pop() ?? filePath;
          flat.push({
            entry: { path: filePath, name, is_dir: false, modified: 0 },
            tag: tagInfo.tag,
          });
          if (flat.length >= 20) break;
        }
        if (flat.length >= 20) break;
      }
      tagResults = flat;
      selectedIndex = 0;
    } finally {
      searching = false;
    }
  }

  function handleInput() {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => doSearch(query), 80);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
      return;
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, resultCount - 1);
      scrollToSelected();
      return;
    }

    if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
      scrollToSelected();
      return;
    }

    if (e.key === "Enter" && resultCount > 0) {
      e.preventDefault();
      if (isTagMode) {
        selectTagResult(tagResults[selectedIndex]);
      } else {
        selectResult(results[selectedIndex]);
      }
      return;
    }
  }

  function scrollToSelected() {
    tick().then(() => {
      const item = listEl?.querySelector(`[data-index="${selectedIndex}"]`);
      item?.scrollIntoView({ block: "nearest" });
    });
  }

  function selectResult(entry: FsEntry) {
    onselect(entry.path);
    onclose();
  }

  function selectTagResult(item: { entry: FsEntry; tag: string }) {
    onselect(item.entry.path);
    onclose();
  }

  onMount(() => {
    inputEl?.focus();
  });

  onDestroy(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="overlay" onclick={onclose} onkeydown={handleKeydown}>
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="switcher"
    role="dialog"
    aria-modal="true"
    aria-label="Quick switcher"
    onclick={(e) => e.stopPropagation()}
  >
    <div class="search-row">
      <Search size={16} class="search-icon" />
      <input
        bind:this={inputEl}
        bind:value={query}
        oninput={handleInput}
        placeholder={m.quickswitcher_placeholder()}
        type="text"
        spellcheck="false"
        autocomplete="off"
      />
      <kbd class="shortcut-hint">esc</kbd>
    </div>

    {#if isTagMode && tagResults.length > 0}
      <div class="results" bind:this={listEl}>
        {#each tagResults as item, i}
          {@const folder = parentPath(item.entry)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="result-item"
            class:selected={i === selectedIndex}
            data-index={i}
            onclick={() => selectTagResult(item)}
            onmouseenter={() => {
              selectedIndex = i;
            }}
          >
            <span class="result-icon"><FileText size={16} /></span>
            <span class="result-name">{displayName(item.entry)}</span>
            <span class="result-tag">#{item.tag}</span>
            {#if folder}
              <span class="result-path">{folder}</span>
            {/if}
          </div>
        {/each}
      </div>
    {:else if !isTagMode && results.length > 0}
      <div class="results" bind:this={listEl}>
        {#each results as entry, i}
          {@const Icon = getIcon(entry.path)}
          {@const folder = parentPath(entry)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="result-item"
            class:selected={i === selectedIndex}
            data-index={i}
            onclick={() => selectResult(entry)}
            onmouseenter={() => {
              selectedIndex = i;
            }}
          >
            <span class="result-icon"><Icon size={16} /></span>
            <span class="result-name">{displayName(entry)}</span>
            {#if folder}
              <span class="result-path">{folder}</span>
            {/if}
          </div>
        {/each}
      </div>
    {:else if query.trim() && !searching}
      <div class="empty">
        No {isTagMode ? "files with that tag" : "files"} found
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    justify-content: center;
    padding-top: min(20vh, 140px);
  }

  .switcher {
    width: 560px;
    max-height: 420px;
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow:
      0 16px 64px rgba(0, 0, 0, 0.5),
      0 0 0 1px rgba(255, 255, 255, 0.04);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: fadeIn 0.12s ease-out;
    align-self: flex-start;
  }

  .search-row {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }

  .search-row :global(.search-icon) {
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .search-row input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 0.95rem;
    font-family: var(--font-sans);
    caret-color: var(--accent);
  }

  .search-row input::placeholder {
    color: var(--text-muted);
  }

  .shortcut-hint {
    font-size: 0.7rem;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    padding: 2px 6px;
    font-family: var(--font-sans);
    flex-shrink: 0;
  }

  .results {
    overflow-y: auto;
    padding: 6px;
  }

  .result-item {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: 8px 10px;
    border-radius: var(--radius-xs);
    cursor: pointer;
    min-height: 36px;
  }

  .result-item.selected {
    background: var(--bg-hover);
  }

  .result-icon {
    color: var(--text-muted);
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .result-name {
    color: var(--text-primary);
    font-size: 0.88rem;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .result-path {
    color: var(--text-muted);
    font-size: 0.78rem;
    margin-left: auto;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
  }

  .result-tag {
    font-size: 0.72rem;
    font-family: var(--font-mono);
    color: var(--text-muted);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 1px 6px;
    flex-shrink: 0;
    margin-left: auto;
  }

  .empty {
    padding: 24px 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.88rem;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(-8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
