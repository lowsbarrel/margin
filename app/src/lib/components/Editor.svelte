<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { Editor } from "@tiptap/core";
  import { common, createLowlight } from "lowlight";
  import { createEditorExtensions } from "$lib/editor/extensions";
  import {
    resolveImagePaths,
    unresolveImagePaths,
    resolveWikiEmbeds,
  } from "$lib/editor/image-paths";
  import { transformImagePaths } from "$lib/editor/text-transform-bridge";
  import { isLocalfileUrl, stripLocalfilePrefix } from "$lib/editor/image-url";
  import { insertPastedFile, insertDroppedFile } from "$lib/editor/attachments";
  import { editor as editorStore } from "$lib/stores/editor.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { vault } from "$lib/stores/vault.svelte";
  import { writeFileBytes } from "$lib/fs/bridge";
  import { saveSnapshot } from "$lib/history/bridge";
  import {
    openPath,
    openUrl,
    revealItemInDir,
  } from "@tauri-apps/plugin-opener";
  import ContextMenu from "./ContextMenu.svelte";
  import type { ContextMenuItem } from "./ContextMenu.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { TextSelection } from "@tiptap/pm/state";
  import { drag } from "$lib/stores/drag.svelte";
  import { computePosition, flip, offset, shift } from "@floating-ui/dom";
  import BubbleToolbar from "./BubbleToolbar.svelte";
  import FindReplace from "./FindReplace.svelte";
  import ImageLightbox from "./ImageLightbox.svelte";
  import { validateName } from "$lib/utils/filename";
  import * as m from "$lib/paraglide/messages.js";

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

  // Keep global tiptap reference in sync with the active pane's editor
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

  // When parent signals an external content update, reload content into the live editor
  // instead of destroying and recreating it.
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
        // Save a snapshot if enough time has passed and content actually changed
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

    // Get current file title for comparison
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
      // Revert to current file name
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

  function updateBubbleButtons() {
    if (!tiptap || !bubbleMenuEl) return;
    bubbleMenuEl
      .querySelectorAll<HTMLButtonElement>("[data-cmd]")
      .forEach((btn) => {
        const cmd = btn.dataset.cmd!;
        btn.classList.toggle("is-active", tiptap!.isActive(cmd));
      });
  }

  function hideBubbleMenu() {
    bubbleVisible = false;
    if (!bubbleMenuEl) return;
    bubbleMenuEl.style.left = "-9999px";
    bubbleMenuEl.style.top = "-9999px";
  }

  function getSelectionRect(
    view: Editor["view"],
    from: number,
    to: number,
  ): DOMRect | null {
    const selection = window.getSelection();
    if (selection && selection.rangeCount > 0) {
      const range = selection.getRangeAt(0);
      if (!range.collapsed) {
        const rect = range.getBoundingClientRect();
        if (rect.width > 0 || rect.height > 0) {
          return rect;
        }
      }
    }

    const start = view.coordsAtPos(from);
    const end = view.coordsAtPos(to);
    return DOMRect.fromRect({
      x: Math.min(start.left, end.left),
      y: Math.min(start.top, end.top),
      width: Math.max(end.right, start.right) - Math.min(start.left, end.left),
      height: Math.max(end.bottom, start.bottom) - Math.min(start.top, end.top),
    });
  }

  /** Position and show/hide the bubble toolbar using @floating-ui/dom */
  function updateBubbleMenu() {
    if (!tiptap || !bubbleMenuEl) return;

    const { state, view } = tiptap;
    const { from, to } = state.selection;

    const shouldShow =
      from !== to &&
      !tiptap.isActive("codeBlock") &&
      !state.selection.empty &&
      view.hasFocus();

    if (!shouldShow) {
      hideBubbleMenu();
      return;
    }

    updateBubbleButtons();
    const token = ++bubblePositionToken;

    const selectionRect = getSelectionRect(view, from, to);
    if (!selectionRect) {
      hideBubbleMenu();
      return;
    }

    const virtualEl = {
      getBoundingClientRect() {
        return selectionRect;
      },
    };

    computePosition(virtualEl, bubbleMenuEl, {
      strategy: "fixed",
      placement: "top",
      middleware: [offset(8), flip(), shift({ padding: 8 })],
    })
      .then(({ x, y }) => {
        if (!bubbleMenuEl || token !== bubblePositionToken) return;
        bubbleMenuEl.style.left = `${x}px`;
        bubbleMenuEl.style.top = `${y}px`;
        bubbleVisible = true;
      })
      .catch(() => {
        if (token !== bubblePositionToken) return;
        hideBubbleMenu();
      });
  }

  /** Prevent default on dragover — required for drop events to fire */
  function handleDragOver(event: DragEvent) {
    event.preventDefault();
  }

  /** Move the editor cursor to a screen coordinate */
  function setCursorAtCoords(x: number, y: number) {
    if (!tiptap) return;
    const result = tiptap.view.posAtCoords({ left: x, top: y });
    if (result == null) return;
    const pos = Math.min(result.pos, tiptap.view.state.doc.content.size);
    try {
      const tr = tiptap.view.state.tr.setSelection(
        TextSelection.create(tiptap.view.state.doc, pos),
      );
      tiptap.view.dispatch(tr);
    } catch {
      /* pos may be invalid for non-text nodes */
    }
  }

  /** Insert a file (from sidebar drag) at the current cursor position */
  async function insertFileAtCursor(path: string) {
    if (!tiptap || !vault.vaultPath) return;
    const name = path.split("/").pop() ?? path;
    const ext = name.split(".").pop()?.toLowerCase() ?? "";

    if (ext === "md") {
      // Insert as a proper wiki-link node
      const title = name.slice(0, -3);
      tiptap.chain().insertWikiLink(title).run();
      return;
    }

    if (ext === "canvas") {
      const title = name.slice(0, -7);
      tiptap.chain().insertWikiLink(title).run();
      return;
    }

    if (!attachmentFolder) {
      toast.error(
        m.toast_set_attachment_folder(),
      );
      return;
    }

    try {
      await insertDroppedFile(path, {
        editor: tiptap,
        vaultPath: vault.vaultPath,
        attachmentFolder,
      });
    } catch (err) {
      toast.error(m.toast_insert_file_failed({ error: String(err) }));
    }
  }

  /** mousemove handler while a sidebar file drag is active — keeps editor cursor in sync */
  function handleInternalDragMouseMove(e: MouseEvent) {
    setCursorAtCoords(e.clientX, e.clientY);
  }

  // Add/remove mousemove listener while a file drag is active
  $effect(() => {
    if (drag.active && drag.item?.kind === "file" && container) {
      container.addEventListener("mousemove", handleInternalDragMouseMove);
      return () =>
        container.removeEventListener("mousemove", handleInternalDragMouseMove);
    }
  });

  // React to a pending insert request from the drop handler in +page.svelte
  $effect(() => {
    const pending = drag.pendingInsert;
    if (!pending || !container || !tiptap) return;
    // Check if the drop coords land inside this editor instance
    const rect = container.getBoundingClientRect();
    if (
      pending.x >= rect.left &&
      pending.x <= rect.right &&
      pending.y >= rect.top &&
      pending.y <= rect.bottom
    ) {
      drag.clearPendingInsert();
      setCursorAtCoords(pending.x, pending.y);
      insertFileAtCursor(pending.path);
    }
  });

  /** Resolve a src attribute (localfile URL or relative) to an absolute path */
  function resolveAbsPath(src: string): string | null {
    if (isLocalfileUrl(src)) {
      const tail = stripLocalfilePrefix(src) ?? "";
      return decodeURIComponent(tail);
    }
    if (
      !src.startsWith("http://") &&
      !src.startsWith("https://") &&
      !src.startsWith("data:") &&
      vault.vaultPath
    ) {
      return `${vault.vaultPath}/${src}`;
    }
    return null;
  }

  /** Handle left-click on images (lightbox) and file-embeds (open) */
  function handleLinkClick(event: MouseEvent) {
    // Image click → lightbox
    const img = (event.target as HTMLElement).closest(
      ".editor-wrap img",
    ) as HTMLImageElement | null;
    if (img && container?.contains(img)) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      lightboxSrc = img.src;
      lightboxAlt = img.alt || "Image";
      return;
    }

    // Wiki-link click → open the linked note
    const wikiEl = (event.target as HTMLElement).closest(
      "[data-wiki-link]",
    ) as HTMLElement | null;
    if (wikiEl && container?.contains(wikiEl)) {
      const linkTitle = wikiEl.getAttribute("data-title");
      if (linkTitle) {
        event.preventDefault();
        event.stopPropagation();
        event.stopImmediatePropagation();
        onwikilink?.(linkTitle);
      }
      return;
    }

    // File-embed click → open with default app
    const embed = (event.target as HTMLElement).closest(
      ".file-embed",
    ) as HTMLElement | null;
    if (embed && container?.contains(embed)) {
      const src = embed.getAttribute("data-src");
      if (src && vault.vaultPath) {
        event.preventDefault();
        event.stopPropagation();
        event.stopImmediatePropagation();
        const absPath = `${vault.vaultPath}/${src}`;
        openPath(absPath).catch((err) =>
          toast.error(m.toast_cannot_open_file({ error: String(err) })),
        );
      }
      return;
    }

    const anchor = (event.target as HTMLElement).closest(
      "a[href]",
    ) as HTMLAnchorElement | null;
    if (!anchor || !container?.contains(anchor)) return;

    const href = anchor.getAttribute("href");
    if (!href) return;

    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation();

    if (href.startsWith("http://") || href.startsWith("https://")) {
      openUrl(href).catch((err) =>
        toast.error(m.toast_cannot_open_url({ error: String(err) })),
      );
    } else if (isLocalfileUrl(href)) {
      const tail = stripLocalfilePrefix(href) ?? "";
      const absPath = decodeURIComponent(tail);
      openPath(absPath).catch((err) =>
        toast.error(m.toast_cannot_open_file({ error: String(err) })),
      );
    } else if (vault.vaultPath) {
      const absPath = `${vault.vaultPath}/${href}`;
      openPath(absPath).catch((err) =>
        toast.error(m.toast_cannot_open_file({ error: String(err) })),
      );
    }
  }

  /** Move editor cursor to match Tauri OS drag position */
  function handleTauriDragOver(pos: { x: number; y: number }) {
    setCursorAtCoords(pos.x, pos.y);
  }

  /** Handle Tauri drag-drop events (OS-level file drops) */
  async function handleTauriDrop(
    paths: string[],
    position?: { x: number; y: number },
  ) {
    if (!vault.vaultPath || !tiptap) return;

    // Only handle drops that land on the editor container
    if (position && container) {
      const rect = container.getBoundingClientRect();
      if (position.x < rect.left || position.x > rect.right ||
          position.y < rect.top || position.y > rect.bottom) return;
    }

    // Place cursor at drop position before inserting
    if (position) setCursorAtCoords(position.x, position.y);

    if (!attachmentFolder) {
      toast.error(
        m.toast_set_attachment_folder(),
      );
      return;
    }

    for (const srcPath of paths) {
      const name = srcPath.replace(/\\/g, "/").split("/").pop() ?? srcPath;
      const ext = name.split(".").pop()?.toLowerCase() ?? "";

      if (ext === "md") {
        const title = name.slice(0, -3);
        tiptap.chain().insertWikiLink(title).run();
        continue;
      }

      try {
        await insertDroppedFile(srcPath, {
          editor: tiptap,
          vaultPath: vault.vaultPath,
          attachmentFolder,
        });
      } catch (err) {
        toast.error(m.toast_insert_file_failed({ error: String(err) }));
      }
    }
  }

  /** Handle pasting images/files into the editor */
  function handlePaste(event: ClipboardEvent) {
    if (!attachmentFolder || !vault.vaultPath || !tiptap) return;

    const clipData = event.clipboardData;
    if (!clipData) return;

    const pastedFiles: File[] = [];
    if (clipData.items) {
      for (let i = 0; i < clipData.items.length; i++) {
        const item = clipData.items[i];
        if (item.kind === "file") {
          const file = item.getAsFile();
          if (file) pastedFiles.push(file);
        }
      }
    }
    if (pastedFiles.length === 0 && clipData.files.length > 0) {
      for (let i = 0; i < clipData.files.length; i++) {
        pastedFiles.push(clipData.files[i]);
      }
    }

    // Fallback for Windows screenshot paste (Win+Shift+S): WebView2 sometimes
    // hides the image from the synchronous DataTransfer API but still exposes
    // it via the async Clipboard API. Detect image intent via types.
    const hasImageType =
      pastedFiles.length === 0 &&
      Array.from(clipData.types || []).some((t) => t.startsWith("image/"));

    if (pastedFiles.length === 0 && !hasImageType) return;
    event.preventDefault();
    event.stopPropagation();

    const ed = tiptap;
    const vp = vault.vaultPath;
    const af = attachmentFolder;

    (async () => {
      if (pastedFiles.length === 0 && hasImageType) {
        try {
          const items = await navigator.clipboard.read();
          for (const item of items) {
            for (const type of item.types) {
              if (!type.startsWith("image/")) continue;
              const blob = await item.getType(type);
              const ext = type.split("/")[1]?.split("+")[0] || "png";
              pastedFiles.push(
                new File([blob], `pasted-${Date.now()}.${ext}`, { type }),
              );
            }
          }
        } catch (err) {
          console.error("Clipboard image read failed:", err);
        }
      }

      for (const file of pastedFiles) {
        try {
          await insertPastedFile(file, {
            editor: ed,
            vaultPath: vp,
            attachmentFolder: af,
          });
        } catch (err) {
          console.error("Failed to paste attachment:", err);
        }
      }
    })();
  }

  /** Right-click context menu for images and file embeds */
  function handleEditorContextMenu(event: MouseEvent) {
    const img = (event.target as HTMLElement).closest(
      ".editor-wrap img",
    ) as HTMLImageElement | null;
    const embed = (event.target as HTMLElement).closest(
      ".file-embed",
    ) as HTMLElement | null;

    if (img && container?.contains(img)) {
      event.preventDefault();
      event.stopPropagation();
      const absPath = resolveAbsPath(img.src);
      const items: ContextMenuItem[] = [
        {
          label: m.editor_view_image(),
          onclick: () => {
            lightboxSrc = img.src;
            lightboxAlt = img.alt || "Image";
          },
        },
      ];
      if (absPath) {
        items.push(
          {
            label: m.editor_open_default_app(),
            onclick: () =>
              openPath(absPath).catch((err) =>
                toast.error(m.toast_cannot_open_file({ error: String(err) })),
              ),
          },
          {
            label: m.editor_reveal_in_finder(),
            onclick: () =>
              revealItemInDir(absPath).catch((err) =>
                toast.error(m.toast_cannot_reveal_file({ error: String(err) })),
              ),
          },
        );
      }
      ctxMenu = { x: event.clientX, y: event.clientY, items };
      return;
    }

    if (embed && container?.contains(embed)) {
      event.preventDefault();
      event.stopPropagation();
      const src = embed.getAttribute("data-src");
      if (src && vault.vaultPath) {
        const absPath = `${vault.vaultPath}/${src}`;
        const fileName = embed.getAttribute("data-filename") || "File";
        ctxMenu = {
          x: event.clientX,
          y: event.clientY,
          items: [
            {
              label: m.editor_open_file({ name: fileName }),
              onclick: () =>
                openPath(absPath).catch((err) =>
                  toast.error(m.toast_cannot_open_file({ error: String(err) })),
                ),
            },
            {
              label: m.editor_reveal_in_finder(),
              onclick: () =>
                revealItemInDir(absPath).catch((err) =>
                  toast.error(m.toast_cannot_reveal_file({ error: String(err) })),
                ),
            },
          ],
        };
      }
      return;
    }

    // Table cell context menu
    const tableCell = (event.target as HTMLElement).closest(
      "td, th",
    ) as HTMLElement | null;
    const tableEl = tableCell?.closest("table");
    if (
      tableCell &&
      tableEl &&
      container?.contains(tableEl) &&
      tiptap?.isActive("table")
    ) {
      event.preventDefault();
      event.stopPropagation();
      ctxMenu = {
        x: event.clientX,
        y: event.clientY,
        items: [
          {
            label: "Add Row Above",
            onclick: () => {
              tiptap?.chain().focus().addRowBefore().run();
            },
          },
          {
            label: "Add Row Below",
            onclick: () => {
              tiptap?.chain().focus().addRowAfter().run();
            },
          },
          {
            label: "Add Column Left",
            onclick: () => {
              tiptap?.chain().focus().addColumnBefore().run();
            },
          },
          {
            label: "Add Column Right",
            onclick: () => {
              tiptap?.chain().focus().addColumnAfter().run();
            },
          },
          {
            label: "Delete Row",
            onclick: () => {
              tiptap?.chain().focus().deleteRow().run();
            },
            destructive: true,
          },
          {
            label: "Delete Column",
            onclick: () => {
              tiptap?.chain().focus().deleteColumn().run();
            },
            destructive: true,
          },
          {
            label: "Delete Table",
            onclick: () => {
              tiptap?.chain().focus().deleteTable().run();
            },
            destructive: true,
          },
        ],
      };
      return;
    }
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
        // Throttle bubble menu repositioning to one per frame
        if (!bubbleUpdateRaf) {
          bubbleUpdateRaf = requestAnimationFrame(() => {
            bubbleUpdateRaf = 0;
            updateBubbleMenu();
          });
        }
      },
      onBlur: () => {
        // Hide bubble menu when editor loses focus (unless focusing the toolbar itself)
        blurTimer = setTimeout(() => {
          blurTimer = null;
          if (!bubbleMenuEl?.contains(document.activeElement)) {
            hideBubbleMenu();
          }
        }, 100);
      },
      onFocus: () => {
        // Re-evaluate on focus return
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
    // Paste: capture-phase on container
    container.addEventListener("paste", handlePaste as EventListener, true);
    // Link clicks: capture-phase on document
    document.addEventListener("click", handleLinkClick as EventListener, true);
    // Right-click: contextmenu on container
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
    // Tauri OS-level drag & drop (bypasses webview entirely)
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

  /* ═══════════════════════════════════════════════════
	   Core — ported from margin core.css
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.md-editor) {
    overflow-y: visible;
    padding: 12px 2.5rem 20vh;
    max-width: 750px;
    margin: 0 auto;
    font-family: var(--font-sans);
    font-size: 15px;
    line-height: 1.6;
    font-weight: 400;
    color: var(--text-primary);
    outline: none;
    cursor: text;
    user-select: text;
    -webkit-user-select: text;
    caret-color: var(--text-primary);
  }

  .editor-wrap :global(.md-editor > * + *) {
    margin-top: 0.4em;
  }

  .editor-wrap :global(.md-editor > *:first-child) {
    margin-top: 0;
  }

  /* ── Paragraphs ────────────────────────────────── */
  .editor-wrap :global(p) {
    margin-top: 0.25em;
    margin-bottom: 0.25em;
  }

  /* ── Headings ──────────────────────────────────── */
  .editor-wrap :global(h1),
  .editor-wrap :global(h2),
  .editor-wrap :global(h3),
  .editor-wrap :global(h4),
  .editor-wrap :global(h5),
  .editor-wrap :global(h6) {
    letter-spacing: -0.015em;
    line-height: 1.3;
    font-weight: 700;
    margin-bottom: 4px;
  }

  .editor-wrap :global(h1) {
    font-size: 1.625em;
    margin-top: 1.5em;
  }

  .editor-wrap :global(h2) {
    font-size: 1.35em;
    margin-top: 1.25em;
  }

  .editor-wrap :global(h3) {
    font-size: 1.15em;
    margin-top: 1em;
  }

  .editor-wrap :global(h4) {
    font-size: 1.05em;
    margin-top: 0.8em;
  }

  .editor-wrap :global(h5) {
    font-size: 1em;
    margin-top: 0.6em;
    color: var(--text-secondary);
  }

  .editor-wrap :global(h6) {
    font-size: 1em;
    margin-top: 0.6em;
    color: var(--text-muted);
  }

  /* ── Links ─────────────────────────────────────── */
  .editor-wrap :global(a) {
    color: var(--text-primary);
    border-bottom: 0.07em solid var(--border);
    font-weight: 500;
    text-decoration: none;
    cursor: pointer;
  }

  /* ── Strong / Emphasis ─────────────────────────── */
  .editor-wrap :global(strong) {
    font-weight: 600;
  }

  .editor-wrap :global(em) {
    font-style: italic;
  }

  .editor-wrap :global(s) {
    text-decoration: line-through;
    opacity: 0.5;
  }

  /* ── Blockquotes ───────────────────────────────── */
  .editor-wrap :global(blockquote) {
    padding-left: 0.8em;
    border-left: 3px solid var(--border);
    margin: 0.15em 0;
  }

  /* ── Horizontal Rule ───────────────────────────── */
  .editor-wrap :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 1.25em 0;
    cursor: pointer;
  }

  .editor-wrap :global(hr.ProseMirror-selectednode) {
    border-top: 1px solid #68cef8;
  }

  /* ── Selected Node ─────────────────────────────── */
  .editor-wrap :global(.ProseMirror-selectednode) {
    outline: 2px solid #70cff8;
  }

  /* ── Block Drag Handle ─────────────────────────── */
  .editor-wrap :global(.block-drag-handle) {
    position: absolute;
    display: none;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 24px;
    border-radius: 4px;
    cursor: grab;
    color: var(--text-muted);
    opacity: 0.35;
    transition: opacity 0.15s ease, background 0.15s ease;
    user-select: none;
    -webkit-user-select: none;
    z-index: 10;
  }

  .editor-wrap :global(.block-drag-handle:hover) {
    opacity: 0.8;
    background: var(--bg-secondary);
  }

  .editor-wrap :global(.block-drag-handle:active) {
    cursor: grabbing;
    opacity: 1;
  }

  @media print {
    .editor-wrap :global(.block-drag-handle) {
      display: none !important;
    }
  }

  /* ── Selection ─────────────────────────────────── */
  .editor-wrap :global(::selection) {
    background: color-mix(in srgb, var(--accent-link) 25%, transparent);
  }

  /* ── Resize Cursor ─────────────────────────────── */
  .editor-wrap :global(.resize-cursor) {
    cursor: col-resize;
  }

  /* ═══════════════════════════════════════════════════
	   Lists — ported from margin ordered-list.css
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(ul),
  .editor-wrap :global(ol) {
    padding: 0 0 0 1.5em;
    margin-top: 0.15em;
    margin-bottom: 0.15em;
  }

  .editor-wrap :global(ul p),
  .editor-wrap :global(ol p) {
    margin-top: 0;
    margin-bottom: 0;
  }

  .editor-wrap :global(li) {
    margin: 0;
  }

  .editor-wrap :global(ol) {
    list-style-position: outside;
  }

  /* Nested ordered list type cycling */
  .editor-wrap :global(ol ol),
  .editor-wrap :global(ol ul),
  .editor-wrap :global(ul ol) {
    margin-top: 0.1rem;
    margin-bottom: 0.1rem;
  }

  .editor-wrap :global(ol) {
    list-style-type: decimal;
  }
  .editor-wrap :global(ol ol) {
    list-style-type: lower-alpha;
  }
  .editor-wrap :global(ol ol ol) {
    list-style-type: lower-roman;
  }
  .editor-wrap :global(ol ol ol ol) {
    list-style-type: decimal;
  }

  /* ═══════════════════════════════════════════════════
	   Task Lists
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(ul[data-type="taskList"]) {
    list-style: none;
    padding: 0;
    margin: 0.15em 0;
  }

  .editor-wrap :global(ul[data-type="taskList"] p) {
    margin: 0;
  }

  .editor-wrap :global(ul[data-type="taskList"] li) {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 1px 0;
  }

  .editor-wrap :global(ul[data-type="taskList"] li > label) {
    flex: 0 0 auto;
    padding-top: 4px;
    user-select: none;
  }

  .editor-wrap :global(ul[data-type="taskList"] li > div) {
    flex: 1 1 auto;
  }

  .editor-wrap
    :global(ul[data-type="taskList"] li > label input[type="checkbox"]) {
    appearance: none;
    -webkit-appearance: none;
    width: 15px;
    height: 15px;
    border: 1.5px solid var(--text-muted);
    border-radius: 3px;
    cursor: pointer;
    margin: 0;
    background: transparent;
    transition: all 0.12s ease;
    position: relative;
  }

  .editor-wrap
    :global(ul[data-type="taskList"] li > label input[type="checkbox"]:hover) {
    border-color: var(--text-secondary);
  }

  .editor-wrap
    :global(
      ul[data-type="taskList"] li > label input[type="checkbox"]:checked
    ) {
    background: var(--accent-link);
    border-color: var(--accent-link);
  }

  .editor-wrap
    :global(
      ul[data-type="taskList"] li > label input[type="checkbox"]:checked::after
    ) {
    content: "";
    position: absolute;
    left: 3px;
    top: 0px;
    width: 4px;
    height: 8px;
    border: solid #fff;
    border-width: 0 1.5px 1.5px 0;
    transform: rotate(45deg);
  }

  .editor-wrap
    :global(ul[data-type="taskList"] li[data-checked="true"] > div p) {
    text-decoration: line-through;
    opacity: 0.45;
    color: var(--text-muted);
  }

  /* Nested lists inside tasks */
  .editor-wrap :global(ul[data-type="taskList"] li ul li),
  .editor-wrap :global(ul[data-type="taskList"] li ol li) {
    display: list-item;
  }

  .editor-wrap :global(ul[data-type="taskList"] ul[data-type="taskList"] > li) {
    display: flex;
  }

  /* ═══════════════════════════════════════════════════
	   Inline Code — ported from margin code.css
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(:not(pre) > code) {
    font-family: var(--font-mono);
    line-height: 1.6;
    padding: 2px 4px;
    border-radius: 4px;
    margin: 0;
    background: var(--bg-tertiary);
    color: #c678dd;
    font-size: 0.9em;
  }

  /* ═══════════════════════════════════════════════════
	   Code Blocks — ported from margin code.css
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(pre) {
    padding: 12px 16px;
    margin: 4px 0;
    font-family: var(--font-mono);
    border-radius: 6px;
    tab-size: 4;
    background: var(--bg-secondary);
    color: var(--text-primary);
    overflow-x: auto;
  }

  .editor-wrap :global(pre code) {
    color: inherit;
    padding: 0;
    background: none;
    font-size: 0.9em;
    line-height: 1.6;
  }

  /* ── Syntax Highlighting ── */
  .editor-wrap :global(.hljs-comment),
  .editor-wrap :global(.hljs-quote) {
    color: var(--text-muted);
  }

  .editor-wrap :global(.hljs-variable),
  .editor-wrap :global(.hljs-template-variable),
  .editor-wrap :global(.hljs-attribute),
  .editor-wrap :global(.hljs-tag),
  .editor-wrap :global(.hljs-name),
  .editor-wrap :global(.hljs-regexp),
  .editor-wrap :global(.hljs-link),
  .editor-wrap :global(.hljs-selector-id),
  .editor-wrap :global(.hljs-selector-class) {
    color: #e06c75;
  }

  .editor-wrap :global(.hljs-number),
  .editor-wrap :global(.hljs-meta),
  .editor-wrap :global(.hljs-built_in),
  .editor-wrap :global(.hljs-builtin-name),
  .editor-wrap :global(.hljs-literal),
  .editor-wrap :global(.hljs-type),
  .editor-wrap :global(.hljs-params) {
    color: #56b6c2;
  }

  .editor-wrap :global(.hljs-string),
  .editor-wrap :global(.hljs-symbol),
  .editor-wrap :global(.hljs-bullet) {
    color: #98c379;
  }

  .editor-wrap :global(.hljs-title),
  .editor-wrap :global(.hljs-section) {
    color: #e5c07b;
  }

  .editor-wrap :global(.hljs-keyword),
  .editor-wrap :global(.hljs-selector-tag) {
    color: #c678dd;
  }

  .editor-wrap :global(.hljs-emphasis) {
    font-style: italic;
  }

  .editor-wrap :global(.hljs-strong) {
    font-weight: 700;
  }

  /* ═══════════════════════════════════════════════════
	   Tables — ported from margin table.css
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.tableWrapper) {
    margin-top: 1rem;
    margin-bottom: 1rem;
    overflow-x: auto;
  }

  .editor-wrap :global(table) {
    border-collapse: collapse;
    margin: 0;
    table-layout: fixed;
    width: 100%;
  }

  .editor-wrap :global(table td),
  .editor-wrap :global(table th) {
    border: 1px solid var(--border);
    box-sizing: border-box;
    min-width: 1em;
    padding: 3px 5px;
    position: relative;
    vertical-align: top;
  }

  .editor-wrap :global(table td p),
  .editor-wrap :global(table th p) {
    margin: 0;
  }

  .editor-wrap :global(table td p + p),
  .editor-wrap :global(table th p + p) {
    margin-top: 0.75rem;
  }

  .editor-wrap :global(table th) {
    background: var(--bg-secondary);
    font-weight: bold;
    text-align: left;
  }

  .editor-wrap :global(.column-resize-handle) {
    background-color: #adf;
    bottom: -1px;
    position: absolute;
    right: -2px;
    pointer-events: none;
    top: 0;
    width: 4px;
  }

  .editor-wrap :global(.selectedCell::after) {
    background: rgba(200, 200, 255, 0.4);
    content: "";
    left: 0;
    right: 0;
    top: 0;
    bottom: 0;
    pointer-events: none;
    position: absolute;
    z-index: 2;
  }

  /* ═══════════════════════════════════════════════════
	   Highlight — ported from margin highlight.css
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(mark) {
    padding: 1px 3px;
    border-radius: 2px;
    color: var(--text-normal) !important;
  }

  .editor-wrap :global(mark[data-color="#faf594"]) {
    background-color: rgba(234, 179, 8, 0.55) !important;
  }

  .editor-wrap :global(mark[data-color="#98d8f2"]) {
    background-color: rgba(37, 99, 235, 0.55) !important;
  }

  .editor-wrap :global(mark[data-color="#7edb6c"]) {
    background-color: rgba(0, 138, 0, 0.55) !important;
  }

  .editor-wrap :global(mark[data-color="#e0d6ed"]) {
    background-color: rgba(147, 51, 234, 0.55) !important;
  }

  .editor-wrap :global(mark[data-color="#ffc6c2"]) {
    background-color: rgba(224, 0, 0, 0.55) !important;
  }

  .editor-wrap :global(mark[data-color="#f5c8a9"]) {
    background-color: rgba(255, 165, 0, 0.6) !important;
  }

  /* Default highlight (no color) */
  .editor-wrap :global(mark:not([data-color])) {
    background: color-mix(in srgb, var(--warning) 55%, transparent);
  }

  /* ═══════════════════════════════════════════════════
	   Placeholder — ported from margin placeholder.css
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.is-editor-empty:first-child::before) {
    content: attr(data-placeholder);
    float: left;
    color: var(--text-muted);
    pointer-events: none;
    height: 0;
  }

  .editor-wrap :global(.is-empty::before) {
    content: attr(data-placeholder);
    float: left;
    color: var(--text-muted);
    pointer-events: none;
    height: 0;
  }

  .editor-wrap :global(table .is-editor-empty:first-child::before),
  .editor-wrap :global(table .is-empty::before) {
    display: none;
  }

  /* ═══════════════════════════════════════════════════
	   Images
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(img) {
    max-width: 100%;
    height: auto;
    border-radius: var(--radius-sm, 6px);
    cursor: zoom-in;
  }

  .editor-wrap :global(.ProseMirror-selectednode img) {
    outline: 2px solid var(--accent-link, rgb(255, 102, 51));
    outline-offset: 2px;
  }

  /* ═══════════════════════════════════════════════════
	   File Embeds — Obsidian-style file cards
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.file-embed) {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    margin: 6px 0;
    background: var(--bg-secondary);
    border-radius: 8px;
    cursor: pointer;
    user-select: none;
    transition: background 0.12s ease;
    border: 1px solid var(--border);
  }

  .editor-wrap :global(.file-embed:hover) {
    background: var(--bg-tertiary);
  }

  /* ═══════════════════════════════════════════════════
	   Wiki Links — [[Note Title]] inline nodes
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.wiki-link) {
    color: var(--accent-link, rgb(255, 102, 51));
    cursor: pointer;
    border-radius: 3px;
    padding: 0 2px;
    text-decoration: none;
    font-weight: 500;
    transition: background 0.1s;
    display: inline;
  }

  .editor-wrap :global(.wiki-link-icon) {
    display: inline-block;
    width: 16px;
    height: 16px;
    vertical-align: text-bottom;
    margin-right: 2px;
    background: currentColor;
    -webkit-mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z'/%3E%3Cpath d='M14 2v4a2 2 0 0 0 2 2h4'/%3E%3Cpath d='M10 13H8'/%3E%3Cpath d='M16 13H14'/%3E%3Cpath d='M10 17H8'/%3E%3Cpath d='M16 17H14'/%3E%3C/svg%3E");
    mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z'/%3E%3Cpath d='M14 2v4a2 2 0 0 0 2 2h4'/%3E%3Cpath d='M10 13H8'/%3E%3Cpath d='M16 13H14'/%3E%3Cpath d='M10 17H8'/%3E%3Cpath d='M16 17H14'/%3E%3C/svg%3E");
    -webkit-mask-size: contain;
    mask-size: contain;
    -webkit-mask-repeat: no-repeat;
    mask-repeat: no-repeat;
    opacity: 0.65;
    flex-shrink: 0;
  }

  .editor-wrap :global(.wiki-link-title) {
    border-bottom: 0.05em solid color-mix(in srgb, var(--accent-link, rgb(255, 102, 51)) 40%, transparent);
  }

  .editor-wrap :global(.wiki-link:hover) {
    background: color-mix(
      in srgb,
      var(--accent-link, rgb(255, 102, 51)) 12%,
      transparent
    );
  }

  .editor-wrap :global(.wiki-link:hover .wiki-link-title) {
    border-bottom-color: var(--accent-link, rgb(255, 102, 51));
  }

  .editor-wrap :global(.ProseMirror-selectednode .wiki-link),
  .editor-wrap :global(.wiki-link.ProseMirror-selectednode) {
    outline: 2px solid var(--accent-link, rgb(255, 102, 51));
    outline-offset: 1px;
  }

  .editor-wrap :global(.ProseMirror-selectednode .file-embed),
  .editor-wrap :global(.file-embed.ProseMirror-selectednode) {
    outline: 2px solid var(--accent-link, rgb(255, 102, 51));
    outline-offset: 1px;
  }

  .editor-wrap :global(.file-embed-name) {
    flex: 1 1 auto;
    font-size: 0.9em;
    font-weight: 500;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ═══════════════════════════════════════════════════
	   Gapcursor
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.ProseMirror-gapcursor::after) {
    border-top: 1px solid var(--text-primary);
  }

  /* ═══════════════════════════════════════════════════
	   Search Highlights
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.search-match) {
    background: rgba(234, 179, 8, 0.3);
    border-radius: 2px;
  }

  .editor-wrap :global(.search-match-current) {
    background: rgba(234, 179, 8, 0.6);
    outline: 1px solid rgba(234, 179, 8, 0.8);
  }

  /* ═══════════════════════════════════════════════════
	   Bubble Menu Wrapper — positioned by @floating-ui
	   ═══════════════════════════════════════════════════ */
  .bubble-wrapper {
    position: fixed;
    top: -9999px;
    left: -9999px;
    z-index: 50;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.1s ease;
  }

  .bubble-wrapper.visible {
    opacity: 1;
    pointer-events: auto;
  }

  /* ═══════════════════════════════════════════════════
	   Content Drag — drop cursor + dragging state
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.content-drop-cursor) {
    display: block;
    width: 100%;
    height: 3px;
    background: var(--accent-link, rgb(255, 102, 51));
    border-radius: 2px;
    pointer-events: none;
    margin: -1px 0;
    position: relative;
  }

  .editor-wrap :global(.content-drop-cursor::before),
  .editor-wrap :global(.content-drop-cursor::after) {
    content: "";
    position: absolute;
    top: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-link, rgb(255, 102, 51));
    pointer-events: none;
  }

  .editor-wrap :global(.content-drop-cursor::before) {
    left: -4px;
  }

  .editor-wrap :global(.content-drop-cursor::after) {
    right: -4px;
  }

  :global(body.content-dragging) {
    cursor: grabbing !important;
  }

  :global(body.content-dragging *) {
    cursor: grabbing !important;
  }

  /* ═══════════════════════════════════════════════════
	   Details/Toggle Block
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.details-block) {
    position: relative;
    display: flex;
    gap: 4px;
    padding: 4px 8px;
    margin: 8px 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-primary);
  }

  .editor-wrap :global(.details-toggle) {
    flex: 0 0 auto;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    width: 20px;
    height: 20px;
    margin-top: 4px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    border-radius: var(--radius-xs);
    transition: transform 0.15s ease, color 0.1s;
  }

  .editor-wrap :global(.details-toggle:hover) {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .editor-wrap :global(.details-toggle.is-open) {
    transform: rotate(90deg);
  }

  .editor-wrap :global(.details-wrapper) {
    flex: 1 1 auto;
    min-width: 0;
  }

  .editor-wrap :global(.details-inner) {
    display: flex;
    flex-direction: column;
  }

  .editor-wrap :global(.details-block:not(.is-open) .details-inner > :nth-child(n+2)) {
    display: none;
  }

  .editor-wrap :global(.details-summary) {
    font-weight: 600;
    padding: 2px 0;
  }

  .editor-wrap :global(.details-content) {
    padding: 4px 0 2px 0;
  }

  /* ═══════════════════════════════════════════════════
	   Code Block — language selector + copy button
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.code-block-wrapper) {
    position: relative;
    margin: 8px 0;
  }

  .editor-wrap :global(.code-block-toolbar) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-bottom: none;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  }

  .editor-wrap :global(.code-block-wrapper pre) {
    margin: 0 !important;
    border-top-left-radius: 0 !important;
    border-top-right-radius: 0 !important;
  }

  .editor-wrap :global(.code-lang-select) {
    height: 22px;
    padding: 0 4px;
    font-size: 0.72rem;
    font-family: var(--font-mono);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    outline: none;
    cursor: pointer;
    -webkit-appearance: none;
    appearance: none;
    padding-right: 14px;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='5'%3E%3Cpath d='M0 0l4 5 4-5z' fill='%23888'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 3px center;
  }

  .editor-wrap :global(.code-lang-select:hover) {
    border-color: var(--text-secondary);
  }

  .editor-wrap :global(.code-copy-btn) {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 22px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .editor-wrap :global(.code-copy-btn:hover) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  /* ═══════════════════════════════════════════════════
	   Resizable Image
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.image-node-view) {
    position: relative;
    display: inline-block;
    max-width: 100%;
    line-height: 0;
  }

  .editor-wrap :global(.image-node-view[data-align="center"]) {
    display: block;
    margin-left: auto;
    margin-right: auto;
  }

  .editor-wrap :global(.image-node-view[data-align="right"]) {
    display: block;
    margin-left: auto;
    margin-right: 0;
  }

  .editor-wrap :global(.image-node-view[data-align="left"]) {
    display: block;
    margin-left: 0;
    margin-right: auto;
  }

  .editor-wrap :global(.image-node-view img) {
    display: block;
    max-width: 100%;
    cursor: default;
  }

  .editor-wrap :global(.image-resize-handle) {
    position: absolute;
    top: 0;
    width: 8px;
    height: 100%;
    cursor: col-resize;
    z-index: 10;
  }

  .editor-wrap :global(.image-resize-handle::after) {
    content: "";
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 4px;
    height: 40px;
    max-height: 50%;
    border-radius: 2px;
    background: var(--accent-link, rgb(255, 102, 51));
    opacity: 0;
    transition: opacity 0.15s;
  }

  .editor-wrap :global(.image-node-view:hover .image-resize-handle::after),
  .editor-wrap :global(.image-node-view.ProseMirror-selectednode .image-resize-handle::after) {
    opacity: 1;
  }

  .editor-wrap :global(.image-resize-handle-left) {
    left: -4px;
  }

  .editor-wrap :global(.image-resize-handle-left::after) {
    left: 0;
  }

  .editor-wrap :global(.image-resize-handle-right) {
    right: -4px;
  }

  .editor-wrap :global(.image-resize-handle-right::after) {
    right: 0;
  }

  .editor-wrap :global(.image-align-toolbar) {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: 2px;
    padding: 2px 4px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-lg);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.15s;
    z-index: 20;
  }

  .editor-wrap :global(.image-node-view.ProseMirror-selectednode .image-align-toolbar) {
    opacity: 1;
    pointer-events: auto;
  }

  .editor-wrap :global(.image-align-btn) {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .editor-wrap :global(.image-align-btn:hover),
  .editor-wrap :global(.image-align-btn.is-active) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  /* ═══════════════════════════════════════════════════
	   Columns Layout
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.columns-block) {
    display: flex;
    gap: 16px;
    margin: 8px 0;
    min-height: 40px;
  }

  .editor-wrap :global(.columns-block > [data-column]) {
    flex: 1 1 0%;
    min-width: 0;
    border: 1px dashed var(--border);
    border-radius: var(--radius-xs);
    padding: 8px;
  }

  .editor-wrap :global(.columns-block > [data-column]:focus-within) {
    border-color: var(--accent-link, rgb(255, 102, 51));
  }

  /* ═══════════════════════════════════════════════════
	   Status Badge
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.status-badge) {
    display: inline-flex;
    align-items: center;
    padding: 1px 8px;
    border-radius: 3px;
    font-size: 0.78rem;
    font-weight: 600;
    line-height: 1.6;
    white-space: nowrap;
    vertical-align: middle;
    cursor: default;
  }

  .editor-wrap :global(.status-badge[data-color="gray"]) {
    background: rgba(107, 114, 128, 0.15);
    color: #6b7280;
  }

  .editor-wrap :global(.status-badge[data-color="blue"]) {
    background: rgba(59, 130, 246, 0.15);
    color: #3b82f6;
  }

  .editor-wrap :global(.status-badge[data-color="green"]) {
    background: rgba(34, 197, 94, 0.15);
    color: #22c55e;
  }

  .editor-wrap :global(.status-badge[data-color="yellow"]) {
    background: rgba(234, 179, 8, 0.15);
    color: #b45309;
  }

  .editor-wrap :global(.status-badge[data-color="red"]) {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
  }

  .editor-wrap :global(.status-badge[data-color="purple"]) {
    background: rgba(168, 85, 247, 0.15);
    color: #a855f7;
  }

  .editor-wrap :global(.status-popover) {
    position: absolute;
    bottom: calc(100% + 4px);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-lg);
    z-index: 100;
  }

  .editor-wrap :global(.status-input) {
    width: 120px;
    height: 24px;
    padding: 0 6px;
    font-size: 0.78rem;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    outline: none;
  }

  .editor-wrap :global(.status-swatches) {
    display: flex;
    gap: 4px;
  }

  .editor-wrap :global(.status-swatch) {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
  }

  .editor-wrap :global(.status-swatch:hover),
  .editor-wrap :global(.status-swatch.is-active) {
    border-color: var(--text-primary);
  }

  /* ═══════════════════════════════════════════════════
	   Heading Anchor
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.heading-anchor-btn) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    margin-left: 4px;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-secondary);
    cursor: pointer;
    opacity: 0;
    vertical-align: middle;
    transition: opacity 0.15s, background 0.1s;
  }

  .editor-wrap :global(h1:hover .heading-anchor-btn),
  .editor-wrap :global(h2:hover .heading-anchor-btn),
  .editor-wrap :global(h3:hover .heading-anchor-btn),
  .editor-wrap :global(h4:hover .heading-anchor-btn),
  .editor-wrap :global(h5:hover .heading-anchor-btn),
  .editor-wrap :global(h6:hover .heading-anchor-btn) {
    opacity: 1;
  }

  .editor-wrap :global(.heading-anchor-btn:hover) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  @media print {
    .editor-wrap :global(.heading-anchor-btn) {
      display: none;
    }
  }

  /* ═══════════════════════════════════════════════════
	   Table DnD
	   ═══════════════════════════════════════════════════ */
  .editor-wrap :global(.table-drag-handle) {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: var(--radius-xs);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    cursor: grab;
    z-index: 50;
    opacity: 0.7;
    transition: opacity 0.15s, background 0.1s;
  }

  .editor-wrap :global(.table-drag-handle::after) {
    content: "⠿";
    font-size: 10px;
    line-height: 1;
    color: var(--text-secondary);
  }

  .editor-wrap :global(.table-drag-handle:hover) {
    opacity: 1;
    background: var(--bg-hover);
  }

  .editor-wrap :global(.table-drag-handle:active) {
    cursor: grabbing;
  }

  .editor-wrap :global(.table-dnd-preview) {
    padding: 0;
    background-color: rgba(255, 255, 255, 0.3);
    backdrop-filter: blur(2px);
    border-radius: var(--radius-xs);
    z-index: 100;
  }

  .editor-wrap :global(.table-dnd-drop-indicator) {
    background-color: var(--accent-link, rgb(255, 102, 51));
    z-index: 99;
  }

  .editor-wrap:has(:global(.table-dnd-drop-indicator[data-dragging="true"])) :global(.prosemirror-dropcursor-block),
  .editor-wrap:has(:global(.table-dnd-drop-indicator[data-dragging="true"])) :global(.prosemirror-dropcursor-inline) {
    display: none;
  }

</style>
