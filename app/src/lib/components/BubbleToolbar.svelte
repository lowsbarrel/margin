<script lang="ts">
  import type { Editor } from "@tiptap/core";

  interface Props {
    editor: Editor | null;
  }

  let { editor }: Props = $props();
  let toolbarEl: HTMLDivElement;
  let showLinkInput = $state(false);
  let linkUrl = $state("");
  let linkInputEl = $state<HTMLInputElement>();

  function updateActiveStates() {
    if (!editor || !toolbarEl) return;
    toolbarEl
      .querySelectorAll<HTMLButtonElement>("[data-cmd]")
      .forEach((btn) => {
        const cmd = btn.dataset.cmd!;
        btn.classList.toggle("is-active", editor!.isActive(cmd));
      });
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
      case "underline":
        editor.chain().focus().toggleUnderline().run();
        break;
      case "code":
        editor.chain().focus().toggleCode().run();
        break;
      case "highlight":
        editor.chain().focus().toggleHighlight().run();
        break;

      case "link":
        if (editor.isActive("link")) {
          editor.chain().focus().unsetLink().run();
        } else {
          showLinkInput = true;
          linkUrl = "";
          // Focus input on next tick
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
    updateActiveStates();
  }

  function cancelLink() {
    showLinkInput = false;
    linkUrl = "";
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

  let headingValue = $derived.by(() => {
    if (!editor) return "0";
    const lvl = getCurrentHeadingLevel(editor);
    return lvl !== null ? String(lvl) : "0";
  });

  function handleHeadingChange(e: Event) {
    if (!editor) return;
    const val = (e.target as HTMLSelectElement).value;
    if (val === "0") {
      editor.chain().focus().setParagraph().run();
    } else {
      const level = parseInt(val) as 1 | 2 | 3 | 4 | 5 | 6;
      editor.chain().focus().setHeading({ level }).run();
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
        aria-label="Confirm link"
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
      <button class="link-cancel" onclick={cancelLink} aria-label="Cancel link">
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
    <button data-cmd="bold" title="Bold (⌘B)"><b>B</b></button>
    <button data-cmd="italic" title="Italic (⌘I)"><i>I</i></button>
    <button data-cmd="strike" title="Strikethrough (⌘⇧S)"><s>S</s></button>
    <button data-cmd="underline" title="Underline (⌘U)"><u>U</u></button>
    <button data-cmd="code" title="Code (⌘E)"><code>&lt;/&gt;</code></button>
    <button data-cmd="highlight" title="Highlight">
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        ><path d="M12 20h9" /><path
          d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"
        /></svg
      >
    </button>
    <span class="divider"></span>
    <select
      class="heading-select"
      value={headingValue}
      onchange={handleHeadingChange}
      title="Block type"
    >
      <option value="0">Text</option>
      <option value="1">H1</option>
      <option value="2">H2</option>
      <option value="3">H3</option>
      <option value="4">H4</option>
      <option value="5">H5</option>
      <option value="6">H6</option>
    </select>
    <span class="divider"></span>
    <button data-cmd="link" title="Link">
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
</style>
