// Paste cleanup: strips trailing whitespace paragraphs, YAML front matter,
// prevents markdown parsing in code blocks, and normalises localfile:// URLs.

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
          // Let ProseMirror handle paste in code blocks as plain text
          handlePaste: (view, event) => {
            if (this.editor.isActive("codeBlock")) {
              return false;
            }

            // Strip YAML front matter from pasted plain text
            const text = event.clipboardData?.getData("text/plain");
            const html = event.clipboardData?.getData("text/html");
            if (text && !html && YAML_FRONT_MATTER_REGEX.test(text)) {
              const cleaned = text.replace(YAML_FRONT_MATTER_REGEX, "").trimStart();
              if (cleaned !== text) {
                // Can't modify clipboardData — insert cleaned text directly
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
