// Adapted from Docmost's MarkdownClipboard extension.
//
// 1. Strips trailing whitespace-only paragraphs from pasted content.
//    Terminals (GNOME Terminal, etc.) and some apps include trailing
//    whitespace in their HTML clipboard data, which ProseMirror parses
//    as an extra empty paragraph. Inside a list item this creates an
//    orphan empty line that breaks the list structure.
//
// 2. Strips YAML front matter from pasted markdown text.
//
// 3. Prevents markdown parsing inside code blocks.
//
// 4. Normalizes localfile:// image URLs in pasted markdown so that spaces
//    in Windows paths (Margin Vault → Margin%20Vault) don't break parsing.

import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Fragment, Slice } from "@tiptap/pm/model";

const YAML_FRONT_MATTER_REGEX = /^\s*---[\s\S]*?---\s*/;
const IMAGE_WITH_LOCALFILE_URL =
  /!\[([^\]]*)\]\(((?:localfile:\/\/|http:\/\/localfile\.localhost)[^)]+)\)/g;

function normalizePastedMarkdown(text: string): string {
  return text.replace(IMAGE_WITH_LOCALFILE_URL, (_m, alt, url) =>
    `![${alt}](${(url as string).replace(/ /g, "%20")})`,
  );
}

export const PasteCleanup = Extension.create({
  name: "pasteCleanup",
  // Higher priority than tiptap-markdown (50) so our handlePaste runs first
  priority: 60,

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("pasteCleanup"),
        props: {
          // Intercept paste inside code blocks — let ProseMirror handle it
          // as plain text (prevents markdown parsing inside code)
          handlePaste: (view, event) => {
            if (this.editor.isActive("codeBlock")) {
              // Return false = let ProseMirror default handle it (inserts as plain text in code blocks)
              return false;
            }

            // Strip YAML front matter from pasted plain text
            const text = event.clipboardData?.getData("text/plain");
            const html = event.clipboardData?.getData("text/html");
            if (text && !html && YAML_FRONT_MATTER_REGEX.test(text)) {
              const cleaned = text.replace(YAML_FRONT_MATTER_REGEX, "").trimStart();
              if (cleaned !== text) {
                // Create a new clipboard event with the cleaned text
                // We can't modify clipboardData, so we manually set the
                // text via the editor's insertContent
                const { from, to } = view.state.selection;
                const tr = view.state.tr.insertText(cleaned, from, to);
                view.dispatch(tr);
                return true;
              }
            }

            return false;
          },

          transformPastedText: (text) => normalizePastedMarkdown(text),

          transformPasted: (slice) => {
            let { content, openStart, openEnd } = slice;

            while (content.childCount > 1) {
              const lastChild = content.lastChild;
              if (
                lastChild?.type.name === "paragraph" &&
                lastChild.textContent.trim() === ""
              ) {
                const children = [];
                for (let i = 0; i < content.childCount - 1; i++) {
                  children.push(content.child(i));
                }
                content = Fragment.from(children);
              } else {
                break;
              }
            }

            if (content !== slice.content) {
              return new Slice(content, openStart, Math.max(openEnd, 1));
            }

            return slice;
          },
        },
      }),
    ];
  },
});
