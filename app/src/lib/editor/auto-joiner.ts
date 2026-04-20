// Adapted from https://github.com/NiclasDev63/tiptap-extension-auto-joiner - MIT
// and Docmost's implementation.
//
// Automatically joins adjacent nodes of the same type (e.g. two consecutive
// bulletList nodes) after every transaction. This prevents paste and other
// editing operations from creating split lists.

import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { canJoin } from "@tiptap/pm/transform";
import type { NodeType } from "@tiptap/pm/model";
import type { Transaction } from "@tiptap/pm/state";

/**
 * Collect changed ranges from a set of transactions, mapping positions
 * forward through later mappings so every position lands in the latest
 * document space. Then find joinable points within those ranges and join
 * them (in reverse order to preserve earlier positions).
 */
function autoJoin(
  transactions: readonly Transaction[],
  newTr: Transaction,
  nodeTypes: NodeType[],
): boolean {
  let ranges: number[] = [];
  for (const tr of transactions) {
    for (let i = 0; i < tr.mapping.maps.length; i++) {
      const map = tr.mapping.maps[i];
      if (!map) continue;
      for (let j = 0; j < ranges.length; j++) ranges[j] = map.map(ranges[j]!);
      map.forEach((_s, _e, from, to) => ranges.push(from, to));
    }
  }

  const joinable: number[] = [];
  for (let i = 0; i < ranges.length; i += 2) {
    const from = ranges[i]!;
    const to = ranges[i + 1]!;
    const $from = newTr.doc.resolve(from);
    const depth = $from.sharedDepth(to);
    const parent = $from.node(depth);
    for (
      let index = $from.indexAfter(depth), pos = $from.after(depth + 1);
      pos <= to;
      ++index
    ) {
      const after = parent.maybeChild(index);
      if (!after) break;
      if (index && joinable.indexOf(pos) === -1) {
        const before = parent.child(index - 1);
        if (before.type === after.type && nodeTypes.includes(before.type)) {
          joinable.push(pos);
        }
      }
      pos += after.nodeSize;
    }
  }

  let joined = false;
  joinable.sort((a, b) => a - b);
  for (let i = joinable.length - 1; i >= 0; i--) {
    if (canJoin(newTr.doc, joinable[i]!)) {
      newTr.join(joinable[i]!);
      joined = true;
    }
  }

  return joined;
}

export interface AutoJoinerOptions {
  elementsToJoin: string[];
}

export const AutoJoiner = Extension.create<AutoJoinerOptions>({
  name: "autoJoiner",

  addOptions() {
    return {
      elementsToJoin: [],
    };
  },

  addProseMirrorPlugins() {
    const joinableNodes: NodeType[] = [
      this.editor.schema.nodes.bulletList,
      this.editor.schema.nodes.orderedList,
    ];

    for (const element of this.options.elementsToJoin) {
      const nodeType = this.editor.schema.nodes[element];
      if (nodeType) joinableNodes.push(nodeType);
    }

    return [
      new Plugin({
        key: new PluginKey(this.name),
        appendTransaction(transactions, _, newState) {
          const newTr = newState.tr;
          if (autoJoin(transactions, newTr, joinableNodes)) {
            return newTr;
          }
        },
      }),
    ];
  },
});
