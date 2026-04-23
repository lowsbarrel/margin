import type { Editor } from "@tiptap/core";
import { isLocalfileUrl, stripLocalfilePrefix } from "$lib/editor/image-url";
import {
  openPath,
  openUrl,
  revealItemInDir,
} from "@tauri-apps/plugin-opener";
import { toast } from "$lib/stores/toast.svelte";
import type { ContextMenuItem } from "$lib/components/ContextMenu.svelte";
import * as m from "$lib/paraglide/messages.js";

/** Resolve a src attribute (localfile URL or relative) to an absolute path */
export function resolveAbsPath(
  src: string,
  vaultPath: string | null,
): string | null {
  if (isLocalfileUrl(src)) {
    const tail = stripLocalfilePrefix(src) ?? "";
    return decodeURIComponent(tail);
  }
  if (
    !src.startsWith("http://") &&
    !src.startsWith("https://") &&
    !src.startsWith("data:") &&
    vaultPath
  ) {
    return `${vaultPath}/${src}`;
  }
  return null;
}

export interface ClickHandlerOptions {
  vaultPath: string | null;
  onLightbox: (src: string, alt: string) => void;
  onWikiLink?: (title: string) => void;
}

/** Handle left-click on images (lightbox), wiki-links, file-embeds, and anchors */
export function handleEditorClick(
  event: MouseEvent,
  container: HTMLElement,
  opts: ClickHandlerOptions,
): void {
  // Image click → lightbox
  const img = (event.target as HTMLElement).closest(
    ".editor-wrap img",
  ) as HTMLImageElement | null;
  if (img && container?.contains(img)) {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation();
    opts.onLightbox(img.src, img.alt || "Image");
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
      opts.onWikiLink?.(linkTitle);
    }
    return;
  }

  // File-embed click → open with default app
  const embed = (event.target as HTMLElement).closest(
    ".file-embed",
  ) as HTMLElement | null;
  if (embed && container?.contains(embed)) {
    const src = embed.getAttribute("data-src");
    if (src && opts.vaultPath) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      const absPath = `${opts.vaultPath}/${src}`;
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
  } else if (opts.vaultPath) {
    const absPath = `${opts.vaultPath}/${href}`;
    openPath(absPath).catch((err) =>
      toast.error(m.toast_cannot_open_file({ error: String(err) })),
    );
  }
}

export interface ContextMenuOptions {
  vaultPath: string | null;
  onLightbox: (src: string, alt: string) => void;
}

/** Build context menu items for images, file-embeds, and table cells */
export function buildEditorContextMenu(
  event: MouseEvent,
  container: HTMLElement,
  editor: Editor | null,
  opts: ContextMenuOptions,
): { x: number; y: number; items: ContextMenuItem[] } | null {
  const img = (event.target as HTMLElement).closest(
    ".editor-wrap img",
  ) as HTMLImageElement | null;
  const embed = (event.target as HTMLElement).closest(
    ".file-embed",
  ) as HTMLElement | null;

  if (img && container?.contains(img)) {
    event.preventDefault();
    event.stopPropagation();
    const absPath = resolveAbsPath(img.src, opts.vaultPath);
    const items: ContextMenuItem[] = [
      {
        label: m.editor_view_image(),
        onclick: () => opts.onLightbox(img.src, img.alt || "Image"),
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
    return { x: event.clientX, y: event.clientY, items };
  }

  if (embed && container?.contains(embed)) {
    event.preventDefault();
    event.stopPropagation();
    const src = embed.getAttribute("data-src");
    if (src && opts.vaultPath) {
      const absPath = `${opts.vaultPath}/${src}`;
      const fileName = embed.getAttribute("data-filename") || "File";
      return {
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
    editor?.isActive("table")
  ) {
    event.preventDefault();
    event.stopPropagation();
    return {
      x: event.clientX,
      y: event.clientY,
      items: [
        {
          label: "Add Row Above",
          onclick: () => { editor?.chain().focus().addRowBefore().run(); },
        },
        {
          label: "Add Row Below",
          onclick: () => { editor?.chain().focus().addRowAfter().run(); },
        },
        {
          label: "Add Column Left",
          onclick: () => { editor?.chain().focus().addColumnBefore().run(); },
        },
        {
          label: "Add Column Right",
          onclick: () => { editor?.chain().focus().addColumnAfter().run(); },
        },
        {
          label: "Delete Row",
          onclick: () => { editor?.chain().focus().deleteRow().run(); },
          destructive: true,
        },
        {
          label: "Delete Column",
          onclick: () => { editor?.chain().focus().deleteColumn().run(); },
          destructive: true,
        },
        {
          label: "Delete Table",
          onclick: () => { editor?.chain().focus().deleteTable().run(); },
          destructive: true,
        },
      ],
    };
  }

  return null;
}
