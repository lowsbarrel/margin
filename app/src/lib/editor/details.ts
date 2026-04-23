// Collapsible details/toggle block (details + detailsSummary + detailsContent).

import { Node, mergeAttributes, wrappingInputRule } from "@tiptap/core";
import type { Node as PMNode } from "@tiptap/pm/model";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    details: {
      setDetails: () => ReturnType;
      unsetDetails: () => ReturnType;
      toggleDetails: () => ReturnType;
    };
  }
}

export const Details = Node.create({
  name: "details",
  group: "block",
  content: "detailsSummary detailsContent",
  defining: true,
  isolating: true,

  addAttributes() {
    return {
      open: {
        default: true,
        parseHTML: (el) => el.hasAttribute("open"),
        renderHTML: (attrs) => (attrs.open ? { open: "" } : {}),
      },
    };
  },

  parseHTML() {
    return [{ tag: "details" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "details",
      mergeAttributes({ class: "details-block" }, HTMLAttributes),
      0,
    ];
  },

  addCommands() {
    return {
      setDetails:
        () =>
        ({ commands }) => {
          return commands.insertContent({
            type: this.name,
            attrs: { open: true },
            content: [
              { type: "detailsSummary", content: [{ type: "text", text: "Summary" }] },
              { type: "detailsContent", content: [{ type: "paragraph" }] },
            ],
          });
        },
      unsetDetails:
        () =>
        ({ commands }) =>
          commands.lift(this.name),
      toggleDetails:
        () =>
        ({ commands }) => {
          return commands.insertContent({
            type: this.name,
            attrs: { open: true },
            content: [
              { type: "detailsSummary", content: [{ type: "text", text: "Summary" }] },
              { type: "detailsContent", content: [{ type: "paragraph" }] },
            ],
          });
        },
    };
  },

  addInputRules() {
    return [
      wrappingInputRule({
        find: /^:::details\s$/,
        type: this.type,
        getAttributes: () => ({ open: true }),
      }),
    ];
  },

  addKeyboardShortcuts() {
    return {
      "Mod-Alt-d": () => this.editor.commands.setDetails(),
    };
  },

  addNodeView() {
    return ({ node, editor, getPos }) => {
      const dom = document.createElement("div");
      dom.classList.add("details-block");

      const toggleBtn = document.createElement("button");
      toggleBtn.classList.add("details-toggle");
      toggleBtn.contentEditable = "false";
      toggleBtn.type = "button";
      toggleBtn.innerHTML = `<svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>`;

      const wrapper = document.createElement("div");
      wrapper.classList.add("details-wrapper");

      const contentDom = document.createElement("div");
      contentDom.classList.add("details-inner");

      wrapper.appendChild(contentDom);
      dom.appendChild(toggleBtn);
      dom.appendChild(wrapper);

      let isOpen = node.attrs.open !== false;
      const updateOpen = () => {
        dom.classList.toggle("is-open", isOpen);
        toggleBtn.classList.toggle("is-open", isOpen);
      };
      updateOpen();

      toggleBtn.addEventListener("click", (e) => {
        e.preventDefault();
        isOpen = !isOpen;
        updateOpen();
        if (typeof getPos === "function") {
          const pos = getPos();
          if (pos != null) {
            const tr = editor.view.state.tr.setNodeMarkup(pos, undefined, {
              ...node.attrs,
              open: isOpen,
            });
            tr.setMeta("addToHistory", false);
            editor.view.dispatch(tr);
          }
        }
      });

      return { dom, contentDOM: contentDom };
    };
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: any, node: PMNode) {
          state.write(":::details\n");
          state.renderContent(node);
          state.write(":::\n");
          state.closeBlock(node);
        },
        parse: {
          setup(md: any) {
            detailsMarkdownPlugin(md);
          },
        },
      },
    };
  },
});

export const DetailsSummary = Node.create({
  name: "detailsSummary",
  group: "block",
  content: "inline*",
  defining: true,

  parseHTML() {
    return [{ tag: "summary" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "summary",
      mergeAttributes({ class: "details-summary" }, HTMLAttributes),
      0,
    ];
  },

  addKeyboardShortcuts() {
    return {
      Backspace: ({ editor }) => {
        const { state } = editor;
        const { $from } = state.selection;
        if ($from.parentOffset !== 0) return false;

        for (let d = $from.depth; d >= 1; d--) {
          if ($from.node(d).type.name === "details") {
            return editor.commands.lift("details");
          }
        }
        return false;
      },
      Enter: ({ editor }) => {
        const { state } = editor;
        const { $from } = state.selection;

        for (let d = $from.depth; d >= 1; d--) {
          if ($from.node(d).type.name === "detailsSummary") {
            // Move cursor to the content area
            const detailsDepth = d - 1;
            const detailsNode = $from.node(detailsDepth);
            if (detailsNode.type.name === "details" && detailsNode.childCount > 1) {
              const contentStart = $from.after(d) + 1;
              editor.commands.setTextSelection(contentStart);
              return true;
            }
          }
        }
        return false;
      },
    };
  },
});

export const DetailsContent = Node.create({
  name: "detailsContent",
  group: "block",
  content: "block+",
  defining: true,

  parseHTML() {
    return [{ tag: 'div[data-details-content]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "div",
      mergeAttributes({ class: "details-content", "data-details-content": "" }, HTMLAttributes),
      0,
    ];
  },

  addKeyboardShortcuts() {
    return {
      Enter: ({ editor }) => {
        const { state } = editor;
        const { $from } = state.selection;
        if (!state.selection.empty) return false;

        // At empty last block in detailsContent — exit the details
        for (let d = $from.depth; d >= 1; d--) {
          if ($from.node(d).type.name === "detailsContent") {
            const contentNode = $from.node(d);
            const lastChild = contentNode.lastChild;
            if (
              lastChild &&
              lastChild.content.size === 0 &&
              $from.index(d) === contentNode.childCount - 1 &&
              $from.parentOffset === 0
            ) {
              // Delete the empty block and create a paragraph after the details
              const detailsDepth = d - 1;
              if ($from.node(detailsDepth)?.type.name === "details") {
                const emptyBlockPos = $from.before($from.depth);
                const emptyBlockEnd = $from.after($from.depth);
                const afterDetails = $from.after(detailsDepth);
                const tr = state.tr
                  .delete(emptyBlockPos, emptyBlockEnd);
                tr.insert(
                    tr.mapping.map(afterDetails),
                    state.schema.nodes.paragraph.create(),
                  );
                const newPos = tr.mapping.map(afterDetails);
                tr.setSelection(
                  // @ts-ignore
                  state.selection.constructor.near(tr.doc.resolve(newPos)),
                );
                editor.view.dispatch(tr);
                return true;
              }
            }
          }
        }
        return false;
      },
    };
  },
});

/** markdown-it plugin for :::details ... ::: fences */
function detailsMarkdownPlugin(md: any) {
  md.block.ruler.before(
    "fence",
    "details",
    function (state: any, startLine: number, endLine: number, silent: boolean) {
      const pos = state.bMarks[startLine] + state.tShift[startLine];
      const max = state.eMarks[startLine];
      const src = state.src;

      if (pos + 10 > max) return false;
      const lineText = src.slice(pos, max).trim();
      if (lineText !== ":::details") return false;

      if (silent) return true;

      let nextLine = startLine + 1;
      let found = false;
      for (; nextLine < endLine; nextLine++) {
        const npos = state.bMarks[nextLine] + state.tShift[nextLine];
        const nmax = state.eMarks[nextLine];
        if (src.slice(npos, nmax).trim() === ":::") {
          found = true;
          break;
        }
      }
      if (!found) return false;

      const openToken = state.push("details_open", "details", 1);
      openToken.attrPush(["open", ""]);
      openToken.map = [startLine, nextLine + 1];

      // First line of content is the summary
      const summaryStart = startLine + 1;
      if (summaryStart < nextLine) {
        state.push("summary_open", "summary", 1);
        const inlineToken = state.push("inline", "", 0);
        const spos = state.bMarks[summaryStart] + state.tShift[summaryStart];
        const smax = state.eMarks[summaryStart];
        inlineToken.content = src.slice(spos, smax);
        inlineToken.children = [];
        state.push("summary_close", "summary", -1);
      }

      // Rest is content
      if (summaryStart + 1 < nextLine) {
        state.push("details_content_open", "div", 1).attrPush(["data-details-content", ""]);

        const oldParent = state.parentType;
        const oldLineMax = state.lineMax;
        state.parentType = "details";
        state.lineMax = nextLine;
        state.md.block.tokenize(state, summaryStart + 1, nextLine);
        state.parentType = oldParent;
        state.lineMax = oldLineMax;

        state.push("details_content_close", "div", -1);
      }

      state.push("details_close", "details", -1);
      state.line = nextLine + 1;
      return true;
    },
  );
}
