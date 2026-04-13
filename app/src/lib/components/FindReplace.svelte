<script lang="ts">
  import type { Editor } from "@tiptap/core";
  import {
    ChevronDown,
    ChevronUp,
    X,
    Replace,
    ReplaceAll,
    CaseSensitive,
  } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  interface Props {
    editor: Editor | null;
    showReplace?: boolean;
    onclose: () => void;
  }

  let { editor, showReplace = false, onclose }: Props = $props();

  let searchInput = $state<HTMLInputElement | null>(null);
  let replaceVisible = $state(false);
  let searchValue = $state("");
  let replaceValue = $state("");
  let caseSensitive = $state(false);
  let totalMatches = $state(0);
  let currentIndex = $state(0);

  function syncMatchState() {
    const storage = (editor?.storage as any)?.searchReplace;
    if (storage) {
      totalMatches = storage.totalMatches ?? 0;
      currentIndex = storage.currentIndex ?? 0;
    } else {
      totalMatches = 0;
      currentIndex = 0;
    }
  }

  $effect(() => {
    if (searchInput) {
      searchInput.focus();
      searchInput.select();
    }
  });

  $effect(() => {
    replaceVisible = showReplace;
  });

  let searchDebounceTimer: ReturnType<typeof setTimeout>;

  function handleSearchInput() {
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(() => {
      if (!editor) return;
      (editor.commands as any).setSearchTerm(searchValue);
      syncMatchState();
    }, 100);
  }

  function handleReplaceInput() {
    if (!editor) return;
    (editor.commands as any).setReplaceTerm(replaceValue);
  }

  function toggleCaseSensitive() {
    caseSensitive = !caseSensitive;
    if (!editor) return;
    (editor.commands as any).setCaseSensitive(caseSensitive);
    syncMatchState();
  }

  function findNext() {
    if (!editor) return;
    (editor.commands as any).findNext();
    syncMatchState();
  }

  function findPrev() {
    if (!editor) return;
    (editor.commands as any).findPrev();
    syncMatchState();
  }

  function replaceCurrent() {
    if (!editor) return;
    (editor.commands as any).setReplaceTerm(replaceValue);
    (editor.commands as any).replaceCurrent();
    syncMatchState();
  }

  function replaceAll() {
    if (!editor) return;
    (editor.commands as any).setReplaceTerm(replaceValue);
    (editor.commands as any).replaceAll();
    syncMatchState();
  }

  function close() {
    clearTimeout(searchDebounceTimer);
    if (editor) {
      (editor.commands as any).clearSearch();
    }
    onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      findNext();
    } else if (e.key === "Enter" && e.shiftKey) {
      e.preventDefault();
      findPrev();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="find-replace-bar" onkeydown={handleKeydown}>
  <div class="find-row">
    <div class="input-group">
      <input
        bind:this={searchInput}
        bind:value={searchValue}
        oninput={handleSearchInput}
        class="find-input"
        placeholder={m.find_placeholder()}
        spellcheck="false"
      />
      <button
        class="toggle-btn"
        class:active={caseSensitive}
        onclick={toggleCaseSensitive}
        title={m.find_case_sensitive()}
      >
        <CaseSensitive size={14} />
      </button>
    </div>
    <span class="match-count">
      {#if searchValue && totalMatches > 0}
        {currentIndex + 1} / {totalMatches}
      {:else if searchValue}
        {m.find_no_results()}
      {/if}
    </span>
    <div class="find-actions">
      <button
        class="action-btn"
        onclick={findPrev}
        title={m.find_previous()}
        disabled={totalMatches === 0}
      >
        <ChevronUp size={16} />
      </button>
      <button
        class="action-btn"
        onclick={findNext}
        title={m.find_next()}
        disabled={totalMatches === 0}
      >
        <ChevronDown size={16} />
      </button>
      <button
        class="action-btn"
        class:active={replaceVisible}
        onclick={() => (replaceVisible = !replaceVisible)}
        title={m.find_toggle_replace()}
      >
        <Replace size={14} />
      </button>
      <button class="action-btn" onclick={close} title={m.find_close()}>
        <X size={16} />
      </button>
    </div>
  </div>

  {#if replaceVisible}
    <div class="replace-row">
      <div class="input-group">
        <input
          bind:value={replaceValue}
          oninput={handleReplaceInput}
          class="find-input"
          placeholder={m.find_replace_placeholder()}
          spellcheck="false"
        />
      </div>
      <div class="find-actions">
        <button
          class="action-btn"
          onclick={replaceCurrent}
          title={m.find_replace()}
          disabled={totalMatches === 0}
        >
          <Replace size={14} />
        </button>
        <button
          class="action-btn"
          onclick={replaceAll}
          title={m.find_replace_all()}
          disabled={totalMatches === 0}
        >
          <ReplaceAll size={14} />
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .find-replace-bar {
    position: absolute;
    top: 8px;
    right: 16px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px 8px;
    box-shadow: var(--shadow-lg);
    min-width: 320px;
  }

  .find-row,
  .replace-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .input-group {
    display: flex;
    align-items: center;
    flex: 1;
    overflow: hidden;
  }

  .find-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 0.9rem;
    font-family: var(--font-sans);
    padding: 4px 8px;
    caret-color: var(--accent);
  }

  .find-input::placeholder {
    color: var(--text-muted);
  }

  .match-count {
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
    min-width: 50px;
    text-align: center;
  }

  .find-actions {
    display: flex;
    gap: 2px;
  }

  .action-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-muted);
    cursor: pointer;
    transition:
      color 0.12s,
      background 0.12s;
  }

  .action-btn:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .action-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .action-btn.active {
    color: var(--text-primary);
    background: var(--bg-tertiary);
  }

  .toggle-btn {
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
    transition:
      color 0.12s,
      background 0.12s;
  }

  .toggle-btn:hover {
    color: var(--text-primary);
  }

  .toggle-btn.active {
    color: var(--accent-link);
    background: var(--bg-tertiary);
  }
</style>
