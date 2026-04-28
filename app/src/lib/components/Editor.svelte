<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { Editor } from "@tiptap/core";
  import { common, createLowlight } from "lowlight";
  import { createEditorExtensions } from "$lib/editor/extensions";
  import { unresolveImagePaths } from "$lib/editor/image-paths";
  import { transformImagePaths } from "$lib/editor/text-transform-bridge";
  import { editor as editorStore } from "$lib/stores/editor.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { writeFileBytes } from "$lib/fs/bridge";
  import { saveSnapshot } from "$lib/history/bridge";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { drag } from "$lib/stores/drag.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import type { ContextMenuItem } from "./ContextMenu.svelte";
  import BubbleToolbar from "./BubbleToolbar.svelte";
  import FindReplace from "./FindReplace.svelte";
  import ImageLightbox from "./ImageLightbox.svelte";
  import { validateName } from "$lib/utils/filename";
  import * as m from "$lib/paraglide/messages.js";
  import "$lib/editor/editor-styles.css";
  import { handleEditorPaste } from "$lib/editor/handlers/paste";
  import { handleEditorClick, buildEditorContextMenu } from "$lib/editor/handlers/clicks";
  import { setCursorAtCoords, insertFileAtCursor, handleTauriFileDrop } from "$lib/editor/handlers/drag-drop";
  import { positionBubbleMenu, updateBubbleButtons } from "$lib/editor/handlers/bubble-menu";

  interface Props {
    filePath: string;
    initialContent: string;
    externalContentVersion?: number;
    title: string;
    active?: boolean;
    onrename?: (oldPath: string, newPath: string) => void;
    onwikilink?: (title: string) => void;
    onsave?: (content: string) => void;
    attachmentFolder?: string | null;
  }

  let {
    filePath,
    initialContent,
    externalContentVersion = 0,
    title: initialTitle,
    active = true,
    onrename,
    onwikilink,
    onsave,
    attachmentFolder = null,
  }: Props = $props();

  let container: HTMLDivElement;
  let bubbleMenuEl: HTMLDivElement;
  let titleEl: HTMLDivElement;
  let tiptap = $state<Editor | null>(null);

  $effect(() => {
    if (active && tiptap) {
      editorStore.setTiptap(tiptap);
    }
  });
  let bubbleVisible = $state(false);
  let bubblePositionToken = 0;
  let bubbleUpdateRaf = 0;

  let renameTimer: ReturnType<typeof setTimeout> | null = null;
  let blurTimer: ReturnType<typeof setTimeout> | null = null;
  let alive = true;
  let currentPath = $state(untrack(() => filePath));
  let unlistenDragDrop: (() => void) | null = null;
  let handleFindHotkeyRef: EventListener | null = null;
  let lightboxSrc = $state<string | null>(null);
  let lightboxAlt = $state("");
  let ctxMenu = $state<{
    x: number;
    y: number;
    items: ContextMenuItem[];
  } | null>(null);
  let showFindReplace = $state(false);
  let findReplaceMode = $state(false);
  const lowlight = createLowlight(common);

  const RENAME_DELAY = 150;
  const SNAPSHOT_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes between snapshots
  let lastSnapshotTime = 0;
  let lastSnapshotMd: string | null = null;

  // Reload content on external update
  let lastSeenVersion = untrack(() => externalContentVersion);
  $effect(() => {
    const v = externalContentVersion;
    if (v !== lastSeenVersion) {
      lastSeenVersion = v;
      if (tiptap && initialContent != null) {
        transformImagePaths(
          initialContent,
          vault.vaultPath,
          attachmentFolder,
          "resolve",
        ).then((resolved) => {
          tiptap!.commands.setContent(resolved, { emitUpdate: false });
        });
      }
    }
  });

  function saveNow(text: string) {
    if (!alive) return;
    onsave?.(text);
    const encoder = new TextEncoder();
    const encoded = encoder.encode(text);
    writeFileBytes(currentPath, encoded)
      .then(() => {
        editorStore.setDirty(false);
        const now = Date.now();
        if (vault.vaultPath && now - lastSnapshotTime >= SNAPSHOT_INTERVAL_MS && text !== lastSnapshotMd) {
          lastSnapshotTime = now;
          lastSnapshotMd = text;
          saveSnapshot(vault.vaultPath, currentPath, encoded).catch((err) => {
            console.warn("Snapshot save failed:", err);
            toast.error(m.toast_save_snapshot_failed());
          });
        }
      })
      .catch((err) => {
        console.error("Save failed:", err);
        toast.error(m.toast_save_file_failed());
      });
  }

  function handleTitleInput() {
    if (!titleEl || !alive) return;
    const raw = titleEl.textContent?.trim() ?? "";
    if (!raw) return;

    const currentName = currentPath.split("/").pop() ?? "";
    const currentTitle = currentName.endsWith(".md")
      ? currentName.slice(0, -3)
      : currentName;
    if (raw === currentTitle) return;

    if (renameTimer) clearTimeout(renameTimer);
    renameTimer = setTimeout(() => {
      if (!alive) return;
      const error = validateName(raw);
      if (error) {
        toast.error(error);
        return;
      }
      const dir = currentPath.substring(0, currentPath.lastIndexOf("/"));
      const newPath = `${dir}/${raw}.md`;
      if (newPath !== currentPath) {
        const oldPath = currentPath;
        currentPath = newPath;
        onrename?.(oldPath, newPath);
      }
    }, RENAME_DELAY);
  }

  function handleTitleBlur() {
    if (!titleEl) return;
    const raw = titleEl.textContent?.trim() ?? "";
    const error = validateName(raw);
    if (error) {
      const name = currentPath.split("/").pop() ?? "";
      titleEl.textContent = name.endsWith(".md") ? name.slice(0, -3) : name;
    }
  }

  function handleTitleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      tiptap?.commands.focus("start");
    }
  }

  function hideBubbleMenu() {
    bubbleVisible = false;
    if (!bubbleMenuEl) return;
    bubbleMenuEl.style.left = "-9999px";
    bubbleMenuEl.style.top = "-9999px";
  }

  function updateBubbleMenu() {
    if (!tiptap || !bubbleMenuEl) return;
    const token = ++bubblePositionToken;
    positionBubbleMenu(tiptap, bubbleMenuEl).then((result) => {
      if (token !== bubblePositionToken) return;
      if (result) {
        bubbleMenuEl.style.left = `${result.x}px`;
        bubbleMenuEl.style.top = `${result.y}px`;
        bubbleVisible = true;
      } else {
        hideBubbleMenu();
      }
    });
  }

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
  }

  function handleInternalDragMouseMove(e: MouseEvent) {
    if (tiptap) setCursorAtCoords(tiptap, e.clientX, e.clientY);
  }

  $effect(() => {
    if (drag.active && drag.item?.kind === "file" && container) {
      container.addEventListener("mousemove", handleInternalDragMouseMove);
      return () =>
        container.removeEventListener("mousemove", handleInternalDragMouseMove);
    }
  });

  // Handle pending insert from drop handler in +page.svelte
  $effect(() => {
    const pending = drag.pendingInsert;
    if (!pending || !container || !tiptap) return;
    const rect = container.getBoundingClientRect();
    if (
      pending.x >= rect.left &&
      pending.x <= rect.right &&
      pending.y >= rect.top &&
      pending.y <= rect.bottom
    ) {
      drag.clearPendingInsert();
      setCursorAtCoords(tiptap, pending.x, pending.y);
      insertFileAtCursor(pending.path, tiptap, vault.vaultPath!, attachmentFolder);
    }
  });

  function handleLinkClick(event: MouseEvent) {
    if (!container) return;
    handleEditorClick(event, container, {
      vaultPath: vault.vaultPath,
      onLightbox: (src, alt) => {
        lightboxSrc = src;
        lightboxAlt = alt;
      },
      onWikiLink: onwikilink,
    });
  }

  function handleTauriDragOver(pos: { x: number; y: number }) {
    if (tiptap) setCursorAtCoords(tiptap, pos.x, pos.y);
  }

  async function handleTauriDrop(
    paths: string[],
    position?: { x: number; y: number },
  ) {
    // If this drop originated from our own native drag, flag it and skip
    if (drag.nativeDragActive) {
      drag.markDroppedBackInApp();
      return;
    }
    if (!vault.vaultPath || !tiptap) return;
    await handleTauriFileDrop(
      paths,
      position,
      tiptap,
      container,
      vault.vaultPath,
      attachmentFolder,
    );
  }

  function handlePaste(event: ClipboardEvent) {
    if (!attachmentFolder || !vault.vaultPath || !tiptap) return;
    handleEditorPaste(event, tiptap, vault.vaultPath, attachmentFolder);
  }

  function handleEditorContextMenu(event: MouseEvent) {
    if (!container) return;
    const result = buildEditorContextMenu(event, container, tiptap, {
      vaultPath: vault.vaultPath,
      onLightbox: (src, alt) => {
        lightboxSrc = src;
        lightboxAlt = alt;
      },
    });
    if (result) ctxMenu = result;
  }

  function createEditor(content: string) {
    if (tiptap) {
      tiptap.destroy();
      tiptap = null;
    }

    let lastSavedMd: string | null = null;

    const inst = new Editor({
      element: container,
      extensions: createEditorExtensions({ lowlight, attachmentFolder }),
      content: content,
      editorProps: {
        attributes: {
          class: "md-editor",
          spellcheck: "false",
        },
      },
      onCreate: ({ editor: e }) => {
        const md =
          (e.storage as Record<string, any>).markdown?.getMarkdown?.() ?? "";
        lastSavedMd = unresolveImagePaths(md, vault.vaultPath);
      },
      onUpdate: ({ editor: e }) => {
        const md =
          (e.storage as Record<string, any>).markdown?.getMarkdown?.() ?? "";
        const text = unresolveImagePaths(md, vault.vaultPath);
        if (text === lastSavedMd) return;
        lastSavedMd = text;
        editorStore.setDirty(true);
        saveNow(text);
      },
      onSelectionUpdate: () => {
        if (!tiptap) return;
        const { from } = tiptap.state.selection;
        const text = tiptap.state.doc.textBetween(0, from, "\n");
        const lines = text.split("\n");
        editorStore.setCursor(
          lines.length,
          (lines[lines.length - 1]?.length ?? 0) + 1,
        );
        if (!bubbleUpdateRaf) {
          bubbleUpdateRaf = requestAnimationFrame(() => {
            bubbleUpdateRaf = 0;
            updateBubbleMenu();
          });
        }
      },
      onBlur: () => {
        blurTimer = setTimeout(() => {
          blurTimer = null;
          if (!bubbleMenuEl?.contains(document.activeElement)) {
            hideBubbleMenu();
          }
        }, 100);
      },
      onFocus: () => {
        updateBubbleMenu();
        editorStore.setTiptap(inst);
      },
    });

    tiptap = inst;
    editorStore.setTiptap(inst);
  }

  onMount(() => {
    titleEl.textContent = initialTitle;
    transformImagePaths(
      initialContent,
      vault.vaultPath,
      attachmentFolder,
      "resolve",
    ).then((resolved) => {
      createEditor(resolved);
    });
    container.addEventListener("paste", handlePaste as EventListener, true);
    document.addEventListener("click", handleLinkClick as EventListener, true);
    container.addEventListener(
      "contextmenu",
      handleEditorContextMenu as EventListener,
      true,
    );
    // Find & Replace keyboard shortcuts
    function handleFindHotkey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        findReplaceMode = false;
        showFindReplace = true;
      } else if ((e.metaKey || e.ctrlKey) && e.key === "h") {
        e.preventDefault();
        findReplaceMode = true;
        showFindReplace = true;
      }
    }
    handleFindHotkeyRef = handleFindHotkey as EventListener;
    container.addEventListener("keydown", handleFindHotkeyRef);

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          handleTauriDrop(event.payload.paths, event.payload.position);
        } else if (event.payload.type === "over") {
          handleTauriDragOver(event.payload.position);
        }
      })
      .then((unlisten) => {
        unlistenDragDrop = unlisten;
      });
  });

  onDestroy(() => {
    alive = false;
    if (renameTimer) clearTimeout(renameTimer);
    if (blurTimer) clearTimeout(blurTimer);
    container?.removeEventListener("paste", handlePaste as EventListener, true);
    document.removeEventListener(
      "click",
      handleLinkClick as EventListener,
      true,
    );
    container?.removeEventListener(
      "contextmenu",
      handleEditorContextMenu as EventListener,
      true,
    );
    if (handleFindHotkeyRef) {
      container?.removeEventListener("keydown", handleFindHotkeyRef);
    }
    unlistenDragDrop?.();
    if (showFindReplace && tiptap) {
      (tiptap.commands as any).clearSearch();
    }
    if (editorStore.tiptap === tiptap) {
      editorStore.setTiptap(null);
    }
    tiptap?.destroy();
    tiptap = null;
  });
</script>

<div
  class="bubble-wrapper"
  class:visible={bubbleVisible}
  bind:this={bubbleMenuEl}
>
  <BubbleToolbar editor={tiptap} />
</div>

<div class="editor-container">
  {#if showFindReplace}
    <FindReplace
      editor={tiptap}
      showReplace={findReplaceMode}
      onclose={() => {
        showFindReplace = false;
      }}
    />
  {/if}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="title-input"
    contenteditable="true"
    bind:this={titleEl}
    oninput={handleTitleInput}
    onkeydown={handleTitleKeydown}
    onblur={handleTitleBlur}
    data-placeholder={m.editor_untitled()}
    role="textbox"
    tabindex={0}
  ></div>
  <div class="editor-wrap" bind:this={container}></div>
</div>

{#if lightboxSrc}
  <ImageLightbox
    src={lightboxSrc}
    alt={lightboxAlt}
    onclose={() => (lightboxSrc = null)}
  />
{/if}

{#if ctxMenu}
  <ContextMenu
    x={ctxMenu.x}
    y={ctxMenu.y}
    items={ctxMenu.items}
    onclose={() => (ctxMenu = null)}
  />
{/if}

<style>
  /* ═══════════════════════════════════════════════════
	   Editor Container
	   ═══════════════════════════════════════════════════ */
  .editor-container {
    position: relative;
    flex: 1;
    overflow-y: auto;
    background: var(--bg-primary);
  }

  /* ═══════════════════════════════════════════════════
	   Title Input — margin-style editable title
	   ═══════════════════════════════════════════════════ */
  .title-input {
    max-width: 750px;
    margin: 0 auto;
    padding: 48px 2.5rem 0;
    font-family: var(--font-sans);
    font-size: 2.25rem;
    font-weight: 700;
    line-height: 1.2;
    letter-spacing: -0.02em;
    color: var(--text-primary);
    outline: none;
    word-break: break-word;
    cursor: text;
  }

  .title-input:empty::before {
    content: attr(data-placeholder);
    color: var(--text-muted);
    pointer-events: none;
  }

  .editor-wrap {
    overflow: hidden;
    background: var(--bg-primary);
  }
</style>
