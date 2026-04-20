// Adapted from Docmost's TrailingNode extension.
//
// Ensures the document always ends with a paragraph node. Without this,
// users can get stuck at the end of non-paragraph blocks (e.g. code blocks,
// tables, callouts) with no way to add new content below.

import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";

export const TrailingNode = Extension.create({
  name: "trailingNode",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey(this.name),
        appendTransaction(_transactions, _oldState, newState) {
          const { doc, tr, schema } = newState;
          const lastNode = doc.lastChild;

          // Already ends with a paragraph — nothing to do
          if (!lastNode || lastNode.type === schema.nodes.paragraph) {
            return null;
          }

          // Also allow ending with a heading (common in short docs)
          if (lastNode.type === schema.nodes.heading) {
            return null;
          }

          return tr.insert(
            doc.content.size,
            schema.nodes.paragraph.create(),
          );
        },
      }),
    ];
  },
});
