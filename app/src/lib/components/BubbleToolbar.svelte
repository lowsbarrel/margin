<script lang="ts">
  import type { Editor } from "@tiptap/core";
  import * as m from "$lib/paraglide/messages.js";

  interface Props {
    editor: Editor | null;
  }

  let { editor }: Props = $props();
  let toolbarEl: HTMLDivElement;
  let showLinkInput = $state(false);
  let linkUrl = $state("");
  let linkInputEl = $state<HTMLInputElement>();
  let cachedButtons: HTMLButtonElement[] = [];

  function cacheButtons() {
    if (!toolbarEl) return;
    cachedButtons = Array.from(toolbarEl.querySelectorAll<HTMLButtonElement>("[data-cmd]"));
  }

  function updateActiveStates() {
    if (!editor || !toolbarEl) return;
    if (cachedButtons.length === 0) cacheButtons();
    for (const btn of cachedButtons) {
      const cmd = btn.dataset.cmd!;
      btn.classList.toggle("is-active", editor!.isActive(cmd));
    }
  }

  function handleClick(e: MouseEvent) {
    const btn = (e.target as HTMLElement).closest<HTMLButtonElement>(
      "[data-cmd]",
    );
    if (!btn || !editor) return;
    const cmd = btn.dataset.cmd!;
    switch (cmd) {
      case "bold":
        editor.chain().focus().toggleBold().run();
        break;
      case "italic":
        editor.chain().focus().toggleItalic().run();
        break;
      case "strike":
        editor.chain().focus().toggleStrike().run();
        break;
      case "code":
        editor.chain().focus().toggleMark("code").run();
        break;

      case "link":
        if (editor.isActive("link")) {
          editor.chain().focus().unsetLink().run();
        } else {
          showLinkInput = true;
          linkUrl = "";
          requestAnimationFrame(() => linkInputEl?.focus());
        }
        break;
    }
    updateActiveStates();
  }

  function submitLink() {
    if (linkUrl.trim() && editor) {
      editor.chain().focus().setLink({ href: linkUrl.trim() }).run();
    }
    showLinkInput = false;
    linkUrl = "";
    cachedButtons = [];
    updateActiveStates();
  }

  function cancelLink() {
    showLinkInput = false;
    linkUrl = "";
    cachedButtons = [];
    editor?.commands.focus();
  }

  function handleLinkKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      submitLink();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelLink();
    }
  }

  function getCurrentHeadingLevel(ed: Editor): number | null {
    for (let l = 1; l <= 6; l++) {
      if (ed.isActive("heading", { level: l })) return l;
    }
    return null;
  }

  function getActiveBlockType(ed: Editor): string {
    const lvl = getCurrentHeadingLevel(ed);
    if (lvl !== null) return `h${lvl}`;
    if (ed.isActive("bulletList")) return "bullet";
    if (ed.isActive("orderedList")) return "ordered";
    if (ed.isActive("taskList")) return "task";
    if (ed.isActive("blockquote")) return "quote";
    if (ed.isActive("codeBlock")) return "code";
    if (ed.isActive("callout")) return "callout";
    if (ed.isActive("details")) return "details";
    return "text";
  }

  let blockType = $derived.by(() => {
    if (!editor) return "text";
    return getActiveBlockType(editor);
  });

  function handleBlockChange(e: Event) {
    if (!editor) return;
    const val = (e.target as HTMLSelectElement).value;
    const chain = editor.chain().focus();
    switch (val) {
      case "text": chain.setParagraph().run(); break;
      case "h1": chain.setHeading({ level: 1 }).run(); break;
      case "h2": chain.setHeading({ level: 2 }).run(); break;
      case "h3": chain.setHeading({ level: 3 }).run(); break;
      case "h4": chain.setHeading({ level: 4 }).run(); break;
      case "h5": chain.setHeading({ level: 5 }).run(); break;
      case "h6": chain.setHeading({ level: 6 }).run(); break;
      case "bullet": chain.toggleBulletList().run(); break;
      case "ordered": chain.toggleOrderedList().run(); break;
      case "task": chain.toggleTaskList().run(); break;
      case "quote": chain.toggleBlockquote().run(); break;
      case "code": chain.toggleCodeBlock().run(); break;
      case "callout": chain.toggleCallout({ type: "info" }).run(); break;
      case "details": chain.setDetails().run(); break;
    }
    updateActiveStates();
  }

  export { updateActiveStates };
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="bubble-menu" bind:this={toolbarEl} onclick={handleClick}>
  {#if showLinkInput}
    <div class="link-input-wrap">
      <input
        bind:this={linkInputEl}
        bind:value={linkUrl}
        class="link-input"
        type="url"
        placeholder="https://..."
        onkeydown={handleLinkKeydown}
      />
      <button
        class="link-submit"
        onclick={submitLink}
        aria-label={m.bubble_confirm_link()}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"><polyline points="20 6 9 17 4 12" /></svg
        >
      </button>
      <button class="link-cancel" onclick={cancelLink} aria-label={m.bubble_cancel_link()}>
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          ><line x1="18" y1="6" x2="6" y2="18" /><line
            x1="6"
            y1="6"
            x2="18"
            y2="18"
          /></svg
        >
      </button>
    </div>
  {:else}
    <button data-cmd="bold" title={m.bubble_bold()}><b>B</b></button>
    <button data-cmd="italic" title={m.bubble_italic()}><i>I</i></button>
    <button data-cmd="strike" title={m.bubble_strike()}><s>S</s></button>
    <button data-cmd="code" title={m.bubble_code()}><code>&lt;/&gt;</code></button>
    <span class="divider"></span>
    <select
      class="heading-select"
      value={blockType}
      onchange={handleBlockChange}
      title={m.bubble_block_type()}
    >
      <option value="text">{m.bubble_text()}</option>
      <option value="h1">H1</option>
      <option value="h2">H2</option>
      <option value="h3">H3</option>
      <option value="h4">H4</option>
      <option value="h5">H5</option>
      <option value="h6">H6</option>
      <option value="bullet">• Bullet list</option>
      <option value="ordered">1. Numbered list</option>
      <option value="task">☑ To-do list</option>
      <option value="quote">" Quote</option>
      <option value="code">&lt;/&gt; Code block</option>
      <option value="callout">💡 Callout</option>
      <option value="details">▶ Toggle</option>
    </select>
    <span class="divider"></span>
    <button data-cmd="link" title={m.bubble_link()}>
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        ><path
          d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"
        /><path
          d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
        /></svg
      >
    </button>
  {/if}
</div>

<style>
  .bubble-menu {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px 6px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-lg);
  }

  .bubble-menu button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    font-size: 0.85rem;
    cursor: pointer;
    transition:
      background 0.1s,
      color 0.1s;
  }

  .bubble-menu button:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .bubble-menu button :global(.is-active),
  .bubble-menu :global(button.is-active) {
    background: var(--accent-link);
    color: var(--text-primary);
  }

  .bubble-menu button code {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    background: none;
    color: inherit;
    padding: 0;
  }

  .bubble-menu .divider {
    width: 1px;
    height: 18px;
    background: var(--border);
    margin: 0 3px;
  }

  .link-input-wrap {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .link-input {
    width: 200px;
    height: 26px;
    padding: 0 8px;
    font-size: 0.85rem;
    font-family: var(--font-mono);
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    outline: none;
    caret-color: var(--accent);
  }

  .link-input:focus {
    border-color: var(--accent-link);
  }

  .link-submit,
  .link-cancel {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background 0.1s,
      color 0.1s;
  }

  .link-submit:hover {
    background: var(--bg-hover);
    color: var(--success);
  }

  .link-cancel:hover {
    background: var(--bg-hover);
    color: var(--danger);
  }

  .heading-select {
    height: 26px;
    padding: 0 4px;
    font-size: 0.78rem;
    font-family: inherit;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    outline: none;
    cursor: pointer;
    -webkit-appearance: none;
    appearance: none;
    padding-right: 16px;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6'%3E%3Cpath d='M0 0l5 6 5-6z' fill='%23888'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 4px center;
  }

  .heading-select:hover {
    border-color: var(--text-secondary);
  }

  .heading-select:focus {
    border-color: var(--accent-link);
  }

  /* Selection preserved when editor blurred */
  :global(.selection-preserved) {
    background: Highlight;
    color: HighlightText;
  }
</style>
