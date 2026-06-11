// Clipboard safety. Keeps only CommonMark + GFM inline formatting on the way in
// (paste) and out (copy/cut). Appearance-only marks — text colour / font
// (`textStyle`), highlight, underline, superscript, subscript — and the
// non-Markdown `textAlign` node attribute are stripped, so notes never
// accumulate styling their `.md` form can't represent and that styling never
// rides the clipboard into another app. Bold/italic/strike/code/link are
// CommonMark + GFM and are deliberately preserved.
//
// Also: strips YAML front matter and trailing blank paragraphs from pastes, and
// lets code blocks receive plain text.

import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Fragment, Slice } from "@tiptap/pm/model";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { encodeLocalfileImageSpaces } from "$lib/editor/image-url";

const YAML_FRONT_MATTER_REGEX = /^\s*---[\s\S]*?---\s*/;

// Marks whose only purpose is visual styling with no Markdown representation.
const APPEARANCE_MARKS = new Set([
  "textStyle", // text colour + font family/size (the Color extension lives here)
  "highlight",
  "underline",
  "superscript",
  "subscript",
]);

// Node attributes that style appearance with no Markdown representation.
const APPEARANCE_ATTRS = ["textAlign"];

function stripNodeAppearance(node: ProseMirrorNode): ProseMirrorNode {
  let next = node;

  // 1. Drop appearance-only marks (keep bold/italic/strike/code/link/…).
  if (next.marks.length) {
    const kept = next.marks.filter((mark) => !APPEARANCE_MARKS.has(mark.type.name));
    if (kept.length !== next.marks.length) next = next.mark(kept);
  }

  // 2. Recurse into children so the rebuilt node carries clean content.
  const content = next.content.size ? sanitizeFragment(next.content) : next.content;

  // 3. Reset appearance-only attributes to their schema default.
  let attrs: Record<string, unknown> | null = null;
  for (const attr of APPEARANCE_ATTRS) {
    if (next.attrs && attr in next.attrs) {
      const def = next.type.spec.attrs?.[attr]?.default ?? null;
      if (next.attrs[attr] !== def) {
        attrs ??= { ...next.attrs };
        attrs[attr] = def;
      }
    }
  }

  if (attrs) return next.type.create(attrs, content, next.marks);
  if (content !== next.content) return next.copy(content);
  return next;
}

function sanitizeFragment(fragment: Fragment): Fragment {
  const children: ProseMirrorNode[] = [];
  let changed = false;
  fragment.forEach((node) => {
    const clean = stripNodeAppearance(node);
    if (clean !== node) changed = true;
    children.push(clean);
  });
  return changed ? Fragment.fromArray(children) : fragment;
}

function sanitizeSlice(slice: Slice): Slice {
  const content = sanitizeFragment(slice.content);
  if (content === slice.content) return slice;
  return new Slice(content, slice.openStart, slice.openEnd);
}

function stripTrailingEmptyParagraphs(slice: Slice): Slice {
  let { content } = slice;
  while (content.childCount > 1) {
    const lastChild = content.lastChild;
    if (
      lastChild?.type.name === "paragraph" &&
      lastChild.textContent.trim() === ""
    ) {
      const children: ProseMirrorNode[] = [];
      for (let i = 0; i < content.childCount - 1; i++) children.push(content.child(i));
      content = Fragment.fromArray(children);
    } else {
      break;
    }
  }

  if (content === slice.content) return slice;
  return new Slice(content, slice.openStart, Math.max(slice.openEnd, 1));
}

function normalizePastedMarkdown(text: string): string {
  return encodeLocalfileImageSpaces(text);
}

export const PasteCleanup = Extension.create({
  name: "pasteCleanup",
  // Higher priority than tiptap-markdown (50) so our handlePaste runs first.
  priority: 60,

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("pasteCleanup"),
        props: {
          // Let ProseMirror handle paste in code blocks as plain text.
          handlePaste: (view, event) => {
            if (this.editor.isActive("codeBlock")) {
              return false;
            }

            // Strip YAML front matter from pasted plain text.
            const text = event.clipboardData?.getData("text/plain");
            const html = event.clipboardData?.getData("text/html");
            if (text && !html && YAML_FRONT_MATTER_REGEX.test(text)) {
              const cleaned = text.replace(YAML_FRONT_MATTER_REGEX, "").trimStart();
              if (cleaned !== text) {
                // Can't modify clipboardData — insert cleaned text directly.
                const { from, to } = view.state.selection;
                const tr = view.state.tr.insertText(cleaned, from, to);
                view.dispatch(tr);
                return true;
              }
            }

            return false;
          },

          transformPastedText: (text) => normalizePastedMarkdown(text),

          // Strip non-Markdown appearance, then drop trailing blank paragraphs.
          transformPasted: (slice) =>
            stripTrailingEmptyParagraphs(sanitizeSlice(slice)),

          // Copy/cut leave the editor clean too, so colours, highlights, and
          // alignment never ride along into another app or back into a note.
          transformCopied: (slice) => sanitizeSlice(slice),
        },
      }),
    ];
  },
});
