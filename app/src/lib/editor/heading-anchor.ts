import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { Node } from "@tiptap/pm/model";

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\w\s-]/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .trim();
}

const LINK_ICON = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>`;
const CHECK_ICON = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`;

function buildDecorations(doc: Node): DecorationSet {
  const decorations: Decoration[] = [];

  doc.descendants((node, pos) => {
    if (node.type.name !== "heading") return;

    const text = node.textContent.trim();
    if (!text) return;

    const slug = slugify(text);
    const widgetPos = pos + node.nodeSize - 1;

    const widget = Decoration.widget(widgetPos, () => {
      const btn = document.createElement("button");
      btn.className = "heading-anchor-btn";
      btn.contentEditable = "false";
      btn.type = "button";
      btn.innerHTML = LINK_ICON;
      btn.addEventListener("click", (e) => {
        e.preventDefault();
        e.stopPropagation();
        navigator.clipboard.writeText("#" + slug).then(() => {
          btn.innerHTML = CHECK_ICON;
          setTimeout(() => {
            btn.innerHTML = LINK_ICON;
          }, 1500);
        });
      });
      return btn;
    }, { side: 0, ignoreSelection: true });

    decorations.push(widget);
  });

  return DecorationSet.create(doc, decorations);
}

export const HeadingAnchor = Extension.create({
  name: "headingAnchor",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("headingAnchor"),
        state: {
          init(_, state) {
            return buildDecorations(state.doc);
          },
          apply(tr, old) {
            return tr.docChanged ? buildDecorations(tr.doc) : old;
          },
        },
        props: {
          decorations(state) {
            return this.getState(state);
          },
        },
      }),
    ];
  },
});
