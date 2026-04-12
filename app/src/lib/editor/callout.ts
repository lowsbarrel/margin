import { Node, mergeAttributes, wrappingInputRule } from "@tiptap/core";
import type { Node as PMNode } from "@tiptap/pm/model";

export type CalloutType = "info" | "note" | "success" | "warning" | "danger";

const VALID_TYPES: CalloutType[] = [
  "info",
  "note",
  "success",
  "warning",
  "danger",
];

function getValidCalloutType(value: string | null): CalloutType {
  if (value && VALID_TYPES.includes(value as CalloutType))
    return value as CalloutType;
  return "info";
}

const CALLOUT_ICONS: Record<CalloutType, string> = {
  info: "ℹ",
  note: "📝",
  success: "✅",
  warning: "⚠",
  danger: "🚫",
};

export const calloutInputRegex = /^:::(\w+)?\s$/;

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    callout: {
      setCallout: (attrs?: { type?: CalloutType }) => ReturnType;
      toggleCallout: (attrs?: { type?: CalloutType }) => ReturnType;
      unsetCallout: () => ReturnType;
    };
  }
}

const Callout = Node.create({
  name: "callout",
  group: "block",
  content: "block+",
  defining: true,
  isolating: true,
  draggable: true,

  addAttributes() {
    return {
      type: {
        default: "info",
        parseHTML: (el: HTMLElement) => el.getAttribute("data-callout-type"),
        renderHTML: (attrs: Record<string, any>) => ({
          "data-callout-type": attrs.type,
        }),
      },
    };
  },

  parseHTML() {
    return [{ tag: 'div[data-type="callout"]' }];
  },

  renderHTML({ HTMLAttributes }) {
    const type = getValidCalloutType(HTMLAttributes["data-callout-type"]);
    const icon = CALLOUT_ICONS[type];
    return [
      "div",
      mergeAttributes(
        {
          "data-type": "callout",
          "data-callout-type": type,
          class: `callout callout-${type}`,
        },
        HTMLAttributes,
      ),
      ["div", { class: "callout-indicator", contenteditable: "false" }, icon],
      ["div", { class: "callout-content" }, 0],
    ];
  },

  addCommands() {
    return {
      setCallout:
        (attrs) =>
        ({ commands }) =>
          commands.wrapIn(this.name, attrs),
      toggleCallout:
        (attrs) =>
        ({ commands }) =>
          commands.toggleWrap(this.name, attrs),
      unsetCallout:
        () =>
        ({ commands }) =>
          commands.lift(this.name),
    };
  },

  addInputRules() {
    return [
      wrappingInputRule({
        find: calloutInputRegex,
        type: this.type,
        getAttributes: (match) => ({
          type: getValidCalloutType(match[1]),
        }),
      }),
    ];
  },

  addKeyboardShortcuts() {
    return {
      Backspace: ({ editor }) => {
        const { state } = editor;
        const { selection } = state;
        if (!selection.empty) return false;

        const { $from } = selection;
        if ($from.parentOffset !== 0) return false;

        const calloutDepth = findCalloutDepth($from);
        if (calloutDepth === null) return false;

        const calloutNode = $from.node(calloutDepth);

        // If the callout has a single empty child, delete the whole callout
        if (
          calloutNode.childCount === 1 &&
          calloutNode.firstChild &&
          calloutNode.firstChild.content.size === 0
        ) {
          const calloutPos = $from.before(calloutDepth);
          const tr = state.tr.replaceWith(
            calloutPos,
            calloutPos + calloutNode.nodeSize,
            state.schema.nodes.paragraph.create(),
          );
          editor.view.dispatch(tr);
          return true;
        }

        // At start of the first child inside the callout → lift out
        const firstChildDepth = calloutDepth + 1;
        if ($from.depth >= firstChildDepth && $from.index(calloutDepth) === 0) {
          return editor.commands.lift(this.name);
        }

        return false;
      },
      Delete: ({ editor }) => {
        const { state } = editor;
        const { selection } = state;
        if (!selection.empty) return false;

        const { $from } = selection;
        const calloutDepth = findCalloutDepth($from);
        if (calloutDepth === null) return false;

        const calloutNode = $from.node(calloutDepth);

        // If the callout has a single empty child, delete the whole callout
        if (
          calloutNode.childCount === 1 &&
          calloutNode.firstChild &&
          calloutNode.firstChild.content.size === 0
        ) {
          const calloutPos = $from.before(calloutDepth);
          const tr = state.tr.replaceWith(
            calloutPos,
            calloutPos + calloutNode.nodeSize,
            state.schema.nodes.paragraph.create(),
          );
          editor.view.dispatch(tr);
          return true;
        }

        return false;
      },
    };
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: any, node: PMNode) {
          const type = node.attrs.type || "info";
          state.write(`:::${type}\n`);
          state.renderContent(node);
          state.write(":::\n");
          state.closeBlock(node);
        },
        parse: {
          setup(md: any) {
            calloutMarkdownPlugin(md);
          },
        },
      },
    };
  },
});

function findCalloutDepth($pos: any): number | null {
  for (let d = $pos.depth; d > 0; d--) {
    if ($pos.node(d).type.name === "callout") return d;
  }
  return null;
}

function calloutMarkdownPlugin(md: any) {
  // Block-level fence for :::type ... :::
  md.block.ruler.before(
    "fence",
    "callout",
    function (state: any, startLine: number, endLine: number, silent: boolean) {
      const pos = state.bMarks[startLine] + state.tShift[startLine];
      const max = state.eMarks[startLine];
      const src = state.src;

      // Must start with :::
      if (pos + 3 > max) return false;
      if (
        src.charCodeAt(pos) !== 0x3a ||
        src.charCodeAt(pos + 1) !== 0x3a ||
        src.charCodeAt(pos + 2) !== 0x3a
      )
        return false;

      const typeStr = src.slice(pos + 3, max).trim();

      if (silent) return true;

      // Find closing :::
      let nextLine = startLine + 1;
      let found = false;
      for (; nextLine < endLine; nextLine++) {
        const npos = state.bMarks[nextLine] + state.tShift[nextLine];
        const nmax = state.eMarks[nextLine];
        const line = src.slice(npos, nmax).trim();
        if (line === ":::") {
          found = true;
          break;
        }
      }

      if (!found) return false;

      const type = getValidCalloutType(typeStr || null);

      const openToken = state.push("callout_open", "div", 1);
      openToken.attrPush(["data-type", "callout"]);
      openToken.attrPush(["data-callout-type", type]);
      openToken.map = [startLine, nextLine + 1];

      // Parse inner content
      const oldParent = state.parentType;
      const oldLineMax = state.lineMax;
      state.parentType = "callout";
      state.lineMax = nextLine;

      // Parse content between opening and closing fences
      state.md.block.tokenize(state, startLine + 1, nextLine);

      state.parentType = oldParent;
      state.lineMax = oldLineMax;

      const closeToken = state.push("callout_close", "div", -1);

      state.line = nextLine + 1;
      return true;
    },
  );

  md.renderer.rules.callout_open = function (tokens: any[], idx: number) {
    const token = tokens[idx];
    const type = token.attrGet("data-callout-type") || "info";
    return `<div data-type="callout" data-callout-type="${md.utils.escapeHtml(type)}">`;
  };

  md.renderer.rules.callout_close = function () {
    return "</div>";
  };
}

export default Callout;
